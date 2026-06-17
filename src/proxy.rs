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
use tracing::{error, info};

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

    /// Connect to all configured upstream servers via real rmcp transports.
    ///
    /// Spawns connections in parallel, handles partial failures gracefully.
    pub async fn connect_all(&self) -> HubResult<()> {
        let server_count = self.config.mcp_servers.len();
        info!("Connecting to {} upstream servers via rmcp", server_count);

        if server_count == 0 {
            info!("No MCP servers configured — proxy will start empty");
            return Ok(());
        }

        // Connect to each server in parallel
        let mut handles = Vec::new();
        for (server_name, server_config) in &self.config.mcp_servers {
            let name = server_name.clone();
            let cfg = server_config.clone();
            handles.push(tokio::spawn(async move {
                Self::connect_single_server(&name, &cfg).await
            }));
        }

        let mut connected = 0;
        let mut failed = 0;

        for handle in handles {
            match handle.await {
                Ok(Ok((svc, info, tools, child))) => {
                    // Store client
                    self.mcp_clients.write().await
                        .insert(info.name.clone(), Arc::new(svc));

                    // Store process handle if stdio
                    if let Some(c) = child {
                        self.process_handles.write().await
                            .insert(info.name.clone(), c);
                    }

                    // Register tools with prefixing
                    let mut registry = self.tool_registry.write().await;
                    let mut routing = self.tool_routing.write().await;
                    for tool in &tools {
                        let agg = AggregatedTool::from_rmcp(tool, &info.name);
                        routing.insert(agg.name.clone(), info.name.clone());
                        registry.insert(agg.name.clone(), agg);
                    }

                    let server_name_for_log = info.name.clone();
                    let tool_count = tools.len();
                    self.servers.write().await.insert(info.name.clone(), info);
                    connected += 1;
                    info!(server = %server_name_for_log, tools = tool_count, "Connected");
                }
                Ok(Err(e)) => {
                    failed += 1;
                    error!(error = %e, "Server connection failed");
                }
                Err(join_err) => {
                    failed += 1;
                    error!(error = %join_err, "Join error");
                }
            }
        }

        info!(connected, failed, total = server_count, "Server connections complete");

        if connected == 0 && server_count > 0 {
            return Err(HubError::Config("Failed to connect to any MCP server".into()));
        }

        Ok(())
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
                };

                Ok((svc, info, tools.tools, None))
            }
        }
    }

    /// Get all aggregated tools.
    pub async fn get_all_tools(&self) -> Vec<AggregatedTool> {
        self.tool_registry.read().await.values().cloned().collect()
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
