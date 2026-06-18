//! Core proxy aggregation engine for mcp-hub.
//!
//! Pattern adopted from MCProxy's proxy.rs — connects to multiple upstream
//! MCP servers via real rmcp 1.7 transport connections, aggregates their
//! tools with name prefixing, and routes tool calls.
//!
//! Key patterns:
//! - Server name prefixing with "___" separator (from MCProxy)
//! - BearerAuthClient decorator for transparent auth (from MCProxy)
//! - Parallel connection + graceful degradation (from MCProxy)
//! - Graceful shutdown escalation (from tool-cli + MCProxy)

use crate::config::{HubConfig, ServerConfig};
use crate::error::{HubError, HubResult};
use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::service::{RoleClient, RunningService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Aggregated tool information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedTool {
    pub name: String,
    pub original_name: String,
    pub description: String,
    pub server_name: String,
    pub input_schema: Option<serde_json::Value>,
}

impl AggregatedTool {
    /// Create from an rmcp Tool, applying server name prefixing.
    pub fn from_rmcp(tool: &Tool, server_name: &str) -> Self {
        AggregatedTool {
            name: prefix_tool_name(server_name, &tool.name),
            original_name: tool.name.to_string(),
            description: tool.description.as_ref().map(|d| d.to_string()).unwrap_or_default(),
            server_name: server_name.to_string(),
            input_schema: Some(serde_json::Value::Object(tool.input_schema.as_ref().clone())),
        }
    }
}

/// Information about a connected upstream server.
#[derive(Debug, Clone)]
pub struct ConnectedServerInfo {
    pub name: String,
    pub config: ServerConfig,
    pub connected: bool,
    pub tool_count: usize,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
    pub layer: crate::config::ServerLayer,
}

/// The main proxy server that aggregates multiple MCP servers.
pub struct ProxyServer {
    config: HubConfig,

    /// Connected upstream server metadata.
    servers: RwLock<HashMap<String, ConnectedServerInfo>>,

    /// Active MCP client connections (one per server).
    mcp_clients: RwLock<HashMap<String, Arc<RunningService<RoleClient, ()>>>>,

    /// Child process handles for stdio servers (for cleanup on shutdown).
    process_handles: RwLock<HashMap<String, tokio::process::Child>>,

    /// Aggregated tool registry (prefixed tool name → tool info).
    tool_registry: RwLock<HashMap<String, AggregatedTool>>,

    /// Tool name → server name mapping (for routing).
    tool_routing: RwLock<HashMap<String, String>>,

    shutting_down: RwLock<bool>,
}

impl ProxyServer {
    pub fn new(config: HubConfig) -> Self {
        Self {
            config,
            servers: RwLock::new(HashMap::new()),
            mcp_clients: RwLock::new(HashMap::new()),
            process_handles: RwLock::new(HashMap::new()),
            tool_registry: RwLock::new(HashMap::new()),
            tool_routing: RwLock::new(HashMap::new()),
            shutting_down: RwLock::new(false),
        }
    }

    /// Connect to upstream servers in priority layers.
    ///
    /// All servers connect at startup for maximum stability.
    /// L1 (Critical): 3 retries, blocks. L2 (Standard): 1 retry. L3 (Optional): 0 retries.
    /// Built-in tools (L0) register immediately.
    pub async fn connect_all(&self) -> HubResult<()> {
        use crate::config::ServerLayer;

        let server_count = self.config.mcp_servers.len();
        info!("Connecting to {} servers in priority layers", server_count);

        // L0: Built-in tools — always available immediately
        self.register_builtin_tools().await;

        if server_count == 0 {
            info!("No servers configured — serving built-in tools only");
            return Ok(());
        }

        // Group servers by layer
        let by_layer = |layer: ServerLayer| -> Vec<_> {
            self.config.mcp_servers.iter()
                .filter(|(_, c)| layer_of(c) == layer)
                .collect()
        };

        let critical = by_layer(ServerLayer::Critical);
        let standard = by_layer(ServerLayer::Standard);
        let optional = by_layer(ServerLayer::Optional);

        info!(
            critical = critical.len(),
            standard = standard.len(),
            optional = optional.len(),
            "Layered connection plan"
        );

        // L1: Critical — sync, 3 retries
        let mut critical_ok = 0;
        for (name, config) in &critical {
            match self.connect_with_retry(name, config, 3).await {
                Ok((svc, mut info, tools, child)) => {
                    info.layer = ServerLayer::Critical;
                    self.store_connection(name, svc, &mut info, &tools, child).await;
                    critical_ok += 1;
                    info!(server = %name, tools = tools.len(), "Critical connected");
                }
                Err(e) => {
                    error!(server = %name, error = %e, "Critical FAILED");
                    self.register_failed_server(name, config, ServerLayer::Critical).await;
                }
            }
        }

        // L2: Standard — sync, 1 retry
        for (name, config) in &standard {
            match self.connect_with_retry(name, config, 1).await {
                Ok((svc, mut info, tools, child)) => {
                    info.layer = ServerLayer::Standard;
                    self.store_connection(name, svc, &mut info, &tools, child).await;
                    info!(server = %name, tools = tools.len(), "Standard connected");
                }
                Err(e) => {
                    warn!(server = %name, error = %e, "Standard FAILED");
                    self.register_failed_server(name, config, ServerLayer::Standard).await;
                }
            }
        }

        // L3: Optional — sync, 0 retries
        for (name, config) in &optional {
            match self.connect_with_retry(name, config, 0).await {
                Ok((svc, mut info, tools, child)) => {
                    info.layer = ServerLayer::Optional;
                    self.store_connection(name, svc, &mut info, &tools, child).await;
                    info!(server = %name, tools = tools.len(), "Optional connected");
                }
                Err(e) => {
                    warn!(server = %name, error = %e, "Optional FAILED (non-blocking)");
                    self.register_failed_server(name, config, ServerLayer::Optional).await;
                }
            }
        }

        let connected_count = self.mcp_clients.read().await.len();
        info!(critical_ok, connected = connected_count, total = server_count, "Connection complete");

        if critical_ok == 0 && !critical.is_empty() {
            return Err(HubError::Config("Failed to connect to any critical server".into()));
        }

        Ok(())
    }

    /// Register a failed server with connected=false.
    async fn register_failed_server(&self, name: &str, config: &ServerConfig, layer: crate::config::ServerLayer) {
        let info = ConnectedServerInfo {
            name: name.to_string(), config: config.clone(),
            connected: false, tool_count: 0,
            last_refresh: Some(chrono::Utc::now()),
            layer,
        };
        self.servers.write().await.insert(name.to_string(), info);
    }

    /// Retry connection with exponential backoff.
    async fn connect_with_retry(
        &self,
        name: &str,
        config: &ServerConfig,
        max_retries: u32,
    ) -> HubResult<(
        rmcp::service::RunningService<rmcp::RoleClient, ()>,
        ConnectedServerInfo,
        Vec<rmcp::model::Tool>,
        Option<tokio::process::Child>,
    )> {
        let mut last_err = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = std::time::Duration::from_secs(2u64.pow(attempt - 1));
                warn!(server = %name, attempt, retry_in_s = delay.as_secs(), "Retrying");
                tokio::time::sleep(delay).await;
            }
            match Self::connect_single_server(name, config).await {
                Ok(result) => return Ok(result),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| HubError::Connection {
            server: name.to_string(),
            message: "Max retries exceeded".into(),
        }))
    }

    /// Store a successful connection in the proxy state.
    async fn store_connection(
        &self,
        name: &str,
        svc: rmcp::service::RunningService<rmcp::RoleClient, ()>,
        info: &mut ConnectedServerInfo,
        tools: &[rmcp::model::Tool],
        child: Option<tokio::process::Child>,
    ) {
        info.tool_count = tools.len();
        info.connected = true;
        info.last_refresh = Some(chrono::Utc::now());

        self.mcp_clients.write().await.insert(name.to_string(), Arc::new(svc));
        if let Some(c) = child {
            self.process_handles.write().await.insert(name.to_string(), c);
        }

        let mut registry = self.tool_registry.write().await;
        let mut routing = self.tool_routing.write().await;
        for tool in tools {
            let agg = AggregatedTool::from_rmcp(tool, name);
            routing.insert(agg.name.clone(), name.to_string());
            registry.insert(agg.name.clone(), agg);
        }
        self.servers.write().await.insert(name.to_string(), info.clone());
    }

    /// Get all aggregated tools.
    pub async fn get_all_tools(&self) -> Vec<AggregatedTool> {
        self.tool_registry.read().await.values().cloned().collect()
    }

    /// Register built-in tools that are always available.
    async fn register_builtin_tools(&self) {
        let builtins = vec![
            AggregatedTool {
                name: "mcp-hub___get_current_time".to_string(),
                original_name: "get_current_time".to_string(),
                description: "Get the current time in human-readable format (UTC)".to_string(),
                server_name: "mcp-hub".to_string(),
                input_schema: None,
            },
            AggregatedTool {
                name: "mcp-hub___get_time_iso".to_string(),
                original_name: "get_time_iso".to_string(),
                description: "Get the current time in ISO 8601 format".to_string(),
                server_name: "mcp-hub".to_string(),
                input_schema: None,
            },
        ];

        let mut registry = self.tool_registry.write().await;
        let mut routing = self.tool_routing.write().await;
        for tool in builtins {
            routing.insert(tool.name.clone(), "mcp-hub".to_string());
            registry.insert(tool.name.clone(), tool);
        }

        info!(tools = 2, "Registered built-in tools");
    }

    /// Handle a built-in tool call (server "mcp-hub").
    fn handle_builtin_tool(&self, tool_name: &str) -> HubResult<String> {
        match tool_name {
            "get_current_time" => Ok(crate::tools::time::get_current_time()),
            "get_time_iso" => Ok(crate::tools::time::get_current_time_iso()),
            "get_unix_timestamp" => Ok(crate::tools::time::get_unix_timestamp().to_string()),
            _ => Err(HubError::ToolNotFound(format!(
                "Built-in tool '{}' not found", tool_name
            ))),
        }
    }

    /// Connect to a single server and discover its tools.
    async fn connect_single_server(
        server_name: &str,
        config: &ServerConfig,
    ) -> HubResult<(
        RunningService<RoleClient, ()>,
        ConnectedServerInfo,
        Vec<Tool>,
        Option<tokio::process::Child>,
    )> {
        info!(server = %server_name, "Connecting to upstream server");

        match config {
            ServerConfig::Stdio { .. } => {
                let (svc, child) = crate::transports::stdio::connect_stdio(config, server_name).await?;

                let tools = svc.list_tools(None).await
                    .map_err(|e| HubError::McpService(format!(
                        "list_tools failed for '{}': {}", server_name, e
                    )))?;

                let info = ConnectedServerInfo {
                    name: server_name.to_string(),
                    config: config.clone(),
                    connected: true,
                    tool_count: tools.tools.len(),
                    last_refresh: Some(chrono::Utc::now()),
                    layer: layer_of(config),
                };

                Ok((svc, info, tools.tools, Some(child)))
            }
            ServerConfig::Http { .. } => {
                let svc = crate::transports::http_client::connect_http(config, server_name).await?;

                let tools = svc.list_tools(None).await
                    .map_err(|e| HubError::McpService(format!(
                        "list_tools failed for '{}': {}", server_name, e
                    )))?;

                let info = ConnectedServerInfo {
                    name: server_name.to_string(),
                    config: config.clone(),
                    connected: true,
                    tool_count: tools.tools.len(),
                    last_refresh: Some(chrono::Utc::now()),
                    layer: layer_of(config),
                };

                Ok((svc, info, tools.tools, None))
            }
        }
    }

    /// Get a specific tool by prefixed name.
    pub async fn get_tool(&self, prefixed_name: &str) -> Option<AggregatedTool> {
        self.tool_registry.read().await.get(prefixed_name).cloned()
    }

    /// Route a tool call to the correct upstream server and execute it.
    pub async fn call_tool(
        &self,
        prefixed_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> HubResult<String> {
        // Get server name from tool routing
        let server_name = {
            let routing = self.tool_routing.read().await;
            routing.get(prefixed_name).cloned().ok_or_else(|| {
                HubError::ToolNotFound(format!(
                    "No server found for tool '{}'. Use server_name___tool_name format.",
                    prefixed_name
                ))
            })?
        };

        // Extract original tool name
        let (_, actual_tool_name) = extract_server_from_prefixed(prefixed_name)
            .ok_or_else(|| HubError::InvalidToolFormat(prefixed_name.to_string()))?;

        // Handle built-in tools (server "mcp-hub")
        if server_name == "mcp-hub" {
            return self.handle_builtin_tool(actual_tool_name);
        }

        // Get the MCP client for this server
        let clients = self.mcp_clients.read().await;
        let client = clients.get(&server_name).ok_or_else(|| {
            HubError::ServerNotFound(format!("Server '{}' not connected", server_name))
        })?;

        // Convert arguments to rmcp's JsonObject
        let rmcp_args = arguments
            .and_then(|v| {
                if v.is_object() {
                    serde_json::from_value(v).ok()
                } else {
                    None
                }
            });

        let mut param = CallToolRequestParams::new(actual_tool_name.to_string());
        param.arguments = rmcp_args;

        info!(server = %server_name, tool = %actual_tool_name, "Calling tool");

        let result = client.call_tool(param).await.map_err(|e| {
            HubError::McpService(format!(
                "Tool call '{}' on server '{}' failed: {}",
                actual_tool_name, server_name, e
            ))
        })?;

        // Serialize result to JSON text
        let text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());

        Ok(text)
    }

    /// Route a tool call — returns server name (legacy, for HTTP server routing).
    pub async fn route_tool_call(&self, prefixed_name: &str) -> HubResult<String> {
        let routing = self.tool_routing.read().await;
        routing.get(prefixed_name).cloned().ok_or_else(|| {
            HubError::ToolNotFound(format!(
                "No server found for tool '{}'.",
                prefixed_name
            ))
        })
    }

    /// Get information about connected servers.
    pub async fn get_server_infos(&self) -> Vec<ConnectedServerInfo> {
        self.servers.read().await.values().cloned().collect()
    }

    /// Get the server name from a prefixed tool name.
    pub fn extract_server_and_tool(prefixed: &str) -> Option<(&str, &str)> {
        extract_server_from_prefixed(prefixed)
    }

    /// Check if the proxy is shutting down.
    pub async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.read().await
    }

    /// Initiate graceful shutdown — disconnect clients, kill child processes.
    pub async fn shutdown(&self) {
        info!("Initiating proxy server shutdown");
        *self.shutting_down.write().await = true;

        // Drop all MCP client connections
        self.mcp_clients.write().await.clear();

        // Kill all child processes
        let mut handles = self.process_handles.write().await;
        for (name, child) in handles.drain() {
            crate::transports::stdio::shutdown_stdio_process(
                &name, child, 5,
            ).await;
        }

        info!("Proxy server shutdown complete");
    }
}

/// Prefix a tool name with its server name using "___" separator.
pub fn prefix_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{}___{}", server_name, tool_name)
}

/// Extract the server name and tool name from a prefixed tool name.
pub fn extract_server_from_prefixed(prefixed: &str) -> Option<(&str, &str)> {
    prefixed.find("___").map(|pos| (&prefixed[..pos], &prefixed[pos + 3..]))
}

/// Extract the layer from a ServerConfig.
fn layer_of(config: &crate::config::ServerConfig) -> crate::config::ServerLayer {
    match config {
        crate::config::ServerConfig::Stdio { layer, .. } => layer.clone(),
        crate::config::ServerConfig::Http { layer, .. } => layer.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_tool_name() {
        assert_eq!(prefix_tool_name("github", "create_issue"), "github___create_issue");
        assert_eq!(prefix_tool_name("fs", "read"), "fs___read");
    }

    #[test]
    fn test_extract_server_from_prefixed() {
        assert_eq!(
            extract_server_from_prefixed("github___create_issue"),
            Some(("github", "create_issue"))
        );
        assert_eq!(extract_server_from_prefixed("no_separator"), None);
        assert_eq!(
            extract_server_from_prefixed("srv___tool___with___underscores"),
            Some(("srv", "tool___with___underscores"))
        );
    }

    #[test]
    fn test_roundtrip() {
        let prefixed = prefix_tool_name("my-server", "my_tool_v2");
        let (extracted_server, extracted_tool) =
            extract_server_from_prefixed(&prefixed).unwrap();
        assert_eq!(extracted_server, "my-server");
        assert_eq!(extracted_tool, "my_tool_v2");
    }
}
