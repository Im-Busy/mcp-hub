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
            description: tool
                .description
                .as_ref()
                .map(|d| d.to_string())
                .unwrap_or_default(),
            server_name: server_name.to_string(),
            input_schema: Some(serde_json::Value::Object(
                tool.input_schema.as_ref().clone(),
            )),
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

    /// Child process handles for stdio servers.
    /// Each handle is wrapped in `Arc<RwLock<Option<Child>>>` so the health
    /// monitor can share ownership and mark dead servers via `take()`.
    process_handles:
        RwLock<HashMap<String, Arc<tokio::sync::RwLock<Option<tokio::process::Child>>>>>,

    /// Aggregated tool registry (prefixed tool name → tool info).
    tool_registry: RwLock<HashMap<String, AggregatedTool>>,

    /// Tool name → server name mapping (for routing).
    tool_routing: RwLock<HashMap<String, String>>,

    shutting_down: RwLock<bool>,

    /// Retry attempt counts for dead server reconnection (reset on success).
    retry_counts: RwLock<HashMap<String, u32>>,
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
            retry_counts: RwLock::new(HashMap::new()),
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
            self.config
                .mcp_servers
                .iter()
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
                    self.store_connection(name, svc, &mut info, &tools, child)
                        .await;
                    critical_ok += 1;
                    info!(server = %name, tools = tools.len(), "Critical connected");
                }
                Err(e) => {
                    error!(server = %name, error = %e, "Critical FAILED");
                    self.register_failed_server(name, config, ServerLayer::Critical)
                        .await;
                }
            }
        }

        // L2: Standard — sync, 1 retry
        for (name, config) in &standard {
            match self.connect_with_retry(name, config, 1).await {
                Ok((svc, mut info, tools, child)) => {
                    info.layer = ServerLayer::Standard;
                    self.store_connection(name, svc, &mut info, &tools, child)
                        .await;
                    info!(server = %name, tools = tools.len(), "Standard connected");
                }
                Err(e) => {
                    warn!(server = %name, error = %e, "Standard FAILED");
                    self.register_failed_server(name, config, ServerLayer::Standard)
                        .await;
                }
            }
        }

        // L3: Optional — sync, 0 retries
        for (name, config) in &optional {
            match self.connect_with_retry(name, config, 0).await {
                Ok((svc, mut info, tools, child)) => {
                    info.layer = ServerLayer::Optional;
                    self.store_connection(name, svc, &mut info, &tools, child)
                        .await;
                    info!(server = %name, tools = tools.len(), "Optional connected");
                }
                Err(e) => {
                    warn!(server = %name, error = %e, "Optional FAILED (non-blocking)");
                    self.register_failed_server(name, config, ServerLayer::Optional)
                        .await;
                }
            }
        }

        let connected_count = self.mcp_clients.read().await.len();
        info!(
            critical_ok,
            connected = connected_count,
            total = server_count,
            "Connection complete"
        );

        if critical_ok == 0 && !critical.is_empty() {
            return Err(HubError::Config(
                "Failed to connect to any critical server".into(),
            ));
        }

        Ok(())
    }

    /// Register a failed server with connected=false.
    async fn register_failed_server(
        &self,
        name: &str,
        config: &ServerConfig,
        layer: crate::config::ServerLayer,
    ) {
        let info = ConnectedServerInfo {
            name: name.to_string(),
            config: config.clone(),
            connected: false,
            tool_count: 0,
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

        self.mcp_clients
            .write()
            .await
            .insert(name.to_string(), Arc::new(svc));
        if let Some(c) = child {
            self.process_handles.write().await.insert(
                name.to_string(),
                Arc::new(tokio::sync::RwLock::new(Some(c))),
            );
        }

        let mut registry = self.tool_registry.write().await;
        let mut routing = self.tool_routing.write().await;
        for tool in tools {
            let agg = AggregatedTool::from_rmcp(tool, name);
            routing.insert(agg.name.clone(), name.to_string());
            registry.insert(agg.name.clone(), agg);
        }
        self.servers
            .write()
            .await
            .insert(name.to_string(), info.clone());
    }

    /// Remove all tools for a server from tool_registry and tool_routing.
    ///
    /// Called when a server dies, before attempting reconnection.
    /// [`store_connection()`] re-adds tools on successful reconnect via
    /// `HashMap::insert` (overwrites by key — safe for duplicates).
    ///
    /// Returns the number of tools removed.
    pub async fn remove_server_tools(&self, server_name: &str) -> usize {
        let mut removed = 0usize;

        // Remove from tool_registry — retain only tools from other servers
        {
            let mut registry = self.tool_registry.write().await;
            registry.retain(|_key, tool| {
                if tool.server_name == server_name {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }

        // Remove from tool_routing — retain only routes to other servers
        {
            let mut routing = self.tool_routing.write().await;
            routing.retain(|_key, srv| srv != server_name);
        }

        if removed > 0 {
            info!(server = %server_name, removed, "Removed server tools from registry");
        }

        removed
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
                "Built-in tool '{}' not found",
                tool_name
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
                let (svc, child) =
                    crate::transports::stdio::connect_stdio(config, server_name).await?;

                let tools = svc.list_tools(None).await.map_err(|e| {
                    HubError::McpService(format!("list_tools failed for '{}': {}", server_name, e))
                })?;

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

                let tools = svc.list_tools(None).await.map_err(|e| {
                    HubError::McpService(format!("list_tools failed for '{}': {}", server_name, e))
                })?;

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
        let rmcp_args = arguments.and_then(|v| {
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
            HubError::ToolNotFound(format!("No server found for tool '{}'.", prefixed_name))
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

    /// Access the process handle for a specific server.
    ///
    /// Returns a cloned `Arc` to the `Option<Child>` so callers (integration
    /// tests, health utilities) can inspect or kill the process.  Returns
    /// `None` if the server has no stdio child.
    pub async fn process_handle(
        &self,
        name: &str,
    ) -> Option<Arc<tokio::sync::RwLock<Option<tokio::process::Child>>>> {
        self.process_handles.read().await.get(name).cloned()
    }

    /// Check the health of all managed child processes.
    ///
    /// Iterates through `process_handles`, calls `try_wait()` on each active
    /// child, and marks dead processes by setting the `Option` to `None`.
    /// Returns the count of alive servers and the names of newly-dead servers
    /// (already-dead servers are silently skipped — they're tracked by
    /// `retry_counts`).
    ///
    /// This is the core health-check logic, callable directly for testing.
    pub async fn check_health(&self) -> (usize, Vec<String>) {
        let handles = self.process_handles.read().await;
        let mut alive = 0usize;
        let mut newly_dead = Vec::new();

        for (name, child_lock) in handles.iter() {
            let mut child_opt = child_lock.write().await;
            if let Some(child) = child_opt.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        warn!(server = %name, exit = ?status.code(), "Server process died");
                        *child_opt = None; // Mark as dead
                        newly_dead.push(name.clone());
                    }
                    Ok(None) => {
                        alive += 1; // Still running
                    }
                    Err(e) => {
                        warn!(server = %name, error = %e, "try_wait error");
                        // Treat as dead on I/O error
                        *child_opt = None;
                        newly_dead.push(name.clone());
                    }
                }
            }
            // Already None — not newly dead; skip (tracked by retry_counts)
        }

        info!(alive, newly_dead = newly_dead.len(), "Health check");
        (alive, newly_dead)
    }

    /// Kill a specific server's child process (for crash-resilience testing).
    ///
    /// Returns `true` if the process was found and killed successfully,
    /// `false` if the server has no active child process.
    pub async fn kill_server_process(&self, name: &str) -> bool {
        let handles = self.process_handles.read().await;
        if let Some(lock) = handles.get(name) {
            let mut child_opt = lock.write().await;
            if let Some(child) = child_opt.as_mut() {
                match child.kill().await {
                    Ok(()) => {
                        *child_opt = None;
                        true
                    }
                    Err(e) => {
                        warn!(server = %name, error = %e, "Failed to kill server process");
                        false
                    }
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Spawn a background health monitor task.
    ///
    /// The monitor loops every 30 seconds, calling [`check_health()`] to detect
    /// dead child processes. For each newly-dead server it removes stale tools
    /// via [`remove_server_tools()`], then attempts reconnection with exponential
    /// backoff (2s→32s, max 10 retries). On success the tools are re-registered
    /// via [`store_connection()`] and the retry count resets.
    ///
    /// If the tool count changes after reconnect, an info-level message is logged
    /// so the Tantivy index can be rebuilt (handled by the `on_list_tools` path).
    ///
    /// [`check_health()`]: ProxyServer::check_health
    /// [`remove_server_tools()`]: ProxyServer::remove_server_tools
    /// [`store_connection()`]: ProxyServer::store_connection
    /// [`shutting_down`]: ProxyServer::shutting_down
    pub fn start_health_monitor(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                if this.is_shutting_down().await {
                    break;
                }
                let (_alive, newly_dead) = this.check_health().await;

                for server_name in &newly_dead {
                    // 1. Get the original ServerConfig from ConnectedServerInfo
                    let config = {
                        let servers = this.servers.read().await;
                        servers.get(server_name).map(|info| info.config.clone())
                    };

                    let Some(config) = config else {
                        warn!(server = %server_name, "No config found for dead server — cannot reconnect");
                        continue;
                    };

                    // 1b. Capture old tool count and remove stale tools from registry
                    let old_tool_count = {
                        let servers = this.servers.read().await;
                        servers
                            .get(server_name)
                            .map(|info| info.tool_count)
                            .unwrap_or(0)
                    };
                    this.remove_server_tools(server_name).await;

                    // 2. Check retry count
                    let retry_count = {
                        let counts = this.retry_counts.read().await;
                        counts.get(server_name).copied().unwrap_or(0)
                    };

                    if retry_count >= 10 {
                        warn!(
                            server = %server_name,
                            retries = retry_count,
                            "Max retries exceeded — giving up"
                        );
                        continue;
                    }

                    // 3. Exponential backoff: 2^retry_count * 2, capped at 32s
                    let delay_secs = std::cmp::min(2u64.pow(retry_count) * 2, 32);
                    warn!(
                        server = %server_name,
                        attempt = retry_count + 1,
                        delay_s = delay_secs,
                        "Attempting reconnection"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

                    // 4. Attempt reconnection
                    match ProxyServer::connect_single_server(server_name, &config).await {
                        Ok((svc, mut info, tools, child)) => {
                            let new_tool_count = tools.len();
                            this.store_connection(server_name, svc, &mut info, &tools, child)
                                .await;
                            // Reset retry count on success
                            this.retry_counts
                                .write()
                                .await
                                .insert(server_name.clone(), 0);

                            if old_tool_count != new_tool_count {
                                info!(
                                    server = %server_name,
                                    old = old_tool_count,
                                    new = new_tool_count,
                                    "Tool count changed after reconnect — Tantivy index needs rebuild"
                                );
                            }

                            info!(
                                server = %server_name,
                                tools = new_tool_count,
                                "Reconnected successfully"
                            );
                        }
                        Err(e) => {
                            let new_count = retry_count + 1;
                            this.retry_counts
                                .write()
                                .await
                                .insert(server_name.clone(), new_count);
                            warn!(
                                server = %server_name,
                                error = %e,
                                attempt = new_count,
                                "Reconnection failed"
                            );
                        }
                    }
                }
            }
            info!("Health monitor stopped");
        });
    }

    /// Initiate graceful shutdown — disconnect clients, kill child processes.
    pub async fn shutdown(&self) {
        info!("Initiating proxy server shutdown");
        *self.shutting_down.write().await = true;

        // Drop all MCP client connections
        self.mcp_clients.write().await.clear();

        // Kill all child processes
        let mut handles = self.process_handles.write().await;
        for (name, child_lock) in handles.drain() {
            let mut child_opt = child_lock.write().await;
            if let Some(child) = child_opt.take() {
                crate::transports::stdio::shutdown_stdio_process(&name, child, 5).await;
            }
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
    prefixed
        .find("___")
        .map(|pos| (&prefixed[..pos], &prefixed[pos + 3..]))
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
        assert_eq!(
            prefix_tool_name("github", "create_issue"),
            "github___create_issue"
        );
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
        let (extracted_server, extracted_tool) = extract_server_from_prefixed(&prefixed).unwrap();
        assert_eq!(extracted_server, "my-server");
        assert_eq!(extracted_tool, "my_tool_v2");
    }

    /// Given a ProxyServer with a dead child process in process_handles,
    /// When check_health() runs,
    /// Then the dead process is detected and marked None.
    #[tokio::test]
    async fn test_health_monitor_detects_dead_process() {
        // Spawn a short-lived process that exits immediately
        #[cfg(windows)]
        let child = tokio::process::Command::new("cmd")
            .args(["/c", "exit", "0"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to spawn test process");
        #[cfg(not(windows))]
        let child = tokio::process::Command::new("true")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to spawn test process");

        // Wait for the process to actually exit
        // (short sleep is reliable for "exit 0" / "true")
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Create a proxy and insert the (now-dead) handle
        let config = crate::config::HubConfig::default();
        let proxy = std::sync::Arc::new(ProxyServer::new(config));

        let child_lock = Arc::new(tokio::sync::RwLock::new(Some(child)));
        proxy
            .process_handles
            .write()
            .await
            .insert("test-server".to_string(), child_lock);

        // Run health check
        let (alive, newly_dead) = proxy.check_health().await;

        // Verify the dead process was detected
        assert_eq!(alive, 0, "No alive servers expected");
        assert_eq!(newly_dead.len(), 1, "One newly-dead server expected");
        assert_eq!(
            newly_dead[0], "test-server",
            "Correct server name in dead list"
        );

        // Verify the handle was marked as None
        let handles = proxy.process_handles.read().await;
        let lock = handles
            .get("test-server")
            .expect("Server entry should still exist");
        let child_opt = lock.read().await;
        assert!(
            child_opt.is_none(),
            "Dead child should have been marked as None"
        );
    }

    /// Check if a command exists on the system PATH.
    fn command_exists(cmd: &str) -> bool {
        if cfg!(windows) {
            std::process::Command::new("where")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            std::process::Command::new("which")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }

    /// Helper: build a HubConfig with one stdio server.
    fn config_with_stdio_server(
        name: &str,
        command: &str,
        args: Vec<&str>,
    ) -> crate::config::HubConfig {
        use std::collections::HashMap;
        let mut servers = HashMap::new();
        servers.insert(
            name.to_string(),
            crate::config::ServerConfig::Stdio {
                command: command.to_string(),
                args: args.into_iter().map(String::from).collect(),
                env: HashMap::new(),
                layer: crate::config::ServerLayer::Critical,
            },
        );
        crate::config::HubConfig {
            mcp_servers: servers,
            ..crate::config::HubConfig::default()
        }
    }

    /// Given a ProxyServer connected to a real MCP time server,
    /// When the child process is killed,
    /// Then the health monitor auto-reconnects within ~35s and tools reappear.
    ///
    /// Verifies that the reconnection spawns a NEW child process (different PID)
    /// rather than reusing the dead one, confirming the full kill→detect→retry→
    /// reconnect→re-register cycle.
    #[tokio::test]
    async fn test_auto_reconnect_dead_server() {
        if !command_exists("npx") {
            eprintln!("SKIP: npx not available on PATH");
            return;
        }

        let config = config_with_stdio_server(
            "time",
            "npx",
            vec!["-y", "@guanxiong/mcp-server-time@1.0.0"],
        );
        let proxy = Arc::new(ProxyServer::new(config));
        proxy
            .connect_all()
            .await
            .expect("Should connect to time server");

        // Verify initial state: time server has tools
        let initial_tools: Vec<_> = proxy
            .get_all_tools()
            .await
            .into_iter()
            .filter(|t| t.server_name == "time")
            .collect();
        assert!(
            !initial_tools.is_empty(),
            "Should have tools from time server initially, got 0"
        );

        // Verify initial server is connected
        {
            let servers = proxy.get_server_infos().await;
            let time_srv = servers
                .iter()
                .find(|s| s.name == "time")
                .expect("Should have time server info");
            assert!(
                time_srv.connected,
                "Time server should be connected initially"
            );
        }

        // Record the original child PID so we can verify a NEW process after reconnect
        let old_pid = {
            let handles = proxy.process_handles.read().await;
            let lock = handles
                .get("time")
                .expect("Should have time server process handle");
            let child_opt = lock.read().await;
            child_opt.as_ref().and_then(|c| c.id())
        };
        eprintln!("Original child PID: {:?}", old_pid);

        // Start the health monitor (polls every 30s, reconnects dead servers)
        proxy.start_health_monitor();

        // Kill the child process
        {
            let handles = proxy.process_handles.read().await;
            let lock = handles
                .get("time")
                .expect("Should have time server process handle");
            let mut child_opt = lock.write().await;
            if let Some(child) = child_opt.as_mut() {
                child.kill().await.expect("Should kill child process");
            }
            drop(child_opt);
        }

        eprintln!("Killed time server child process — waiting for auto-reconnect...");

        // Health monitor polls every 30s, then 2^0*2=2s backoff → ~32s worst case.
        // Use 45s timeout for safety margin on slow CI.
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(45);

        // Poll for a DIFFERENT PID (confirms fresh spawn, not stale handle)
        let new_pid = loop {
            if start.elapsed() > timeout {
                panic!(
                    "Timed out waiting for reconnection after {:?}",
                    start.elapsed()
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let handles = proxy.process_handles.read().await;
            if let Some(lock) = handles.get("time") {
                let child_opt = lock.read().await;
                if let Some(child) = child_opt.as_ref() {
                    let current_pid = child.id();
                    // Different PID means a new process was spawned
                    if current_pid.is_some() && current_pid != old_pid {
                        eprintln!(
                            "Reconnected after {:?} — new PID: {:?}",
                            start.elapsed(),
                            current_pid
                        );
                        break current_pid;
                    }
                }
            }
        };

        assert!(
            new_pid.is_some(),
            "Reconnected child should have a valid PID"
        );
        assert_ne!(
            new_pid, old_pid,
            "Reconnected child should have a different PID from old={:?}",
            old_pid
        );

        // Verify server reports as connected
        let servers = proxy.get_server_infos().await;
        let time_server = servers
            .iter()
            .find(|s| s.name == "time")
            .expect("Should have time server info after reconnect");
        assert!(
            time_server.connected,
            "Time server should be connected after reconnect"
        );
        assert!(
            time_server.tool_count > 0,
            "Time server should have tools after reconnect, got {}",
            time_server.tool_count
        );

        // Verify tools from time server exist
        let tools = proxy.get_all_tools().await;
        let time_tools: Vec<_> = tools
            .into_iter()
            .filter(|t| t.server_name == "time")
            .collect();
        assert!(
            !time_tools.is_empty(),
            "Time server tools should reappear after reconnect, got 0"
        );
        eprintln!(
            "Reconnect time tools: {:?}",
            time_tools
                .iter()
                .map(|t| &t.original_name)
                .collect::<Vec<_>>()
        );

        // Cleanup
        proxy.shutdown().await;
    }

    /// Given a ProxyServer with tools for two simulated servers,
    /// When one server dies and remove_server_tools() is called,
    /// Then only that server's tools are removed from registry + routing.
    /// When the server reconnects (tools re-inserted),
    /// Then all tools reappear with no duplicates and correct count.
    #[tokio::test]
    async fn test_tool_registry_consistency_on_reconnect() {
        let config = crate::config::HubConfig::default();
        let proxy = ProxyServer::new(config);

        // --- Phase 1: Insert tools for two simulated servers ---
        let alpha_tools = &["tool_a", "tool_b", "tool_c"];
        let beta_tools = &["tool_x", "tool_y"];

        // Helper: insert tools for a server into registry + routing
        async fn insert_server_tools(proxy: &ProxyServer, server_name: &str, tool_names: &[&str]) {
            let mut registry = proxy.tool_registry.write().await;
            let mut routing = proxy.tool_routing.write().await;
            for &tool_name in tool_names {
                let agg = AggregatedTool {
                    name: prefix_tool_name(server_name, tool_name),
                    original_name: tool_name.to_string(),
                    description: format!("Test tool {tool_name}"),
                    server_name: server_name.to_string(),
                    input_schema: None,
                };
                routing.insert(agg.name.clone(), server_name.to_string());
                registry.insert(agg.name.clone(), agg);
            }
        }

        // Set up server info so the health monitor path is realistic
        {
            let mut servers = proxy.servers.write().await;
            servers.insert(
                "alpha".into(),
                ConnectedServerInfo {
                    name: "alpha".into(),
                    config: crate::config::ServerConfig::Stdio {
                        command: "echo".into(),
                        args: vec![],
                        env: std::collections::HashMap::new(),
                        layer: crate::config::ServerLayer::Standard,
                    },
                    connected: true,
                    tool_count: alpha_tools.len(),
                    last_refresh: Some(chrono::Utc::now()),
                    layer: crate::config::ServerLayer::Standard,
                },
            );
            servers.insert(
                "beta".into(),
                ConnectedServerInfo {
                    name: "beta".into(),
                    config: crate::config::ServerConfig::Stdio {
                        command: "echo".into(),
                        args: vec![],
                        env: std::collections::HashMap::new(),
                        layer: crate::config::ServerLayer::Standard,
                    },
                    connected: true,
                    tool_count: beta_tools.len(),
                    last_refresh: Some(chrono::Utc::now()),
                    layer: crate::config::ServerLayer::Standard,
                },
            );
        }

        insert_server_tools(&proxy, "alpha", alpha_tools).await;
        insert_server_tools(&proxy, "beta", beta_tools).await;

        // Verify initial state
        let all = proxy.get_all_tools().await;
        assert_eq!(all.len(), alpha_tools.len() + beta_tools.len());
        {
            let routing = proxy.tool_routing.read().await;
            assert_eq!(routing.len(), alpha_tools.len() + beta_tools.len());
        }

        // --- Phase 2: "alpha" dies — remove its tools ---
        let removed = proxy.remove_server_tools("alpha").await;
        assert_eq!(removed, alpha_tools.len(), "Should remove all alpha tools");

        // Verify alpha tools gone, beta tools remain
        let after_remove = proxy.get_all_tools().await;
        assert_eq!(after_remove.len(), beta_tools.len());
        for tool in &after_remove {
            assert_eq!(
                tool.server_name, "beta",
                "Only beta tools should remain, found: {}",
                tool.name
            );
        }
        {
            let routing = proxy.tool_routing.read().await;
            assert_eq!(routing.len(), beta_tools.len());
            for srv in routing.values() {
                assert_eq!(srv, "beta", "Only beta routes should remain");
            }
        }

        // --- Phase 3: "alpha" reconnects — re-insert tools ---
        insert_server_tools(&proxy, "alpha", alpha_tools).await;

        // Verify: all tools present, no duplicates, correct count
        let after_reconnect = proxy.get_all_tools().await;
        assert_eq!(
            after_reconnect.len(),
            alpha_tools.len() + beta_tools.len(),
            "All tools should reappear after reconnect, no extras"
        );

        let alpha_count = after_reconnect
            .iter()
            .filter(|t| t.server_name == "alpha")
            .count();
        let beta_count = after_reconnect
            .iter()
            .filter(|t| t.server_name == "beta")
            .count();
        assert_eq!(
            alpha_count,
            alpha_tools.len(),
            "Alpha tool count should match original"
        );
        assert_eq!(
            beta_count,
            beta_tools.len(),
            "Beta tool count should be unchanged"
        );

        // Verify no duplicate keys in registry (HashMap guarantees this — but check)
        {
            let registry = proxy.tool_registry.read().await;
            assert_eq!(
                registry.len(),
                alpha_tools.len() + beta_tools.len(),
                "Registry should have exactly one entry per tool, no duplicates"
            );
        }
        {
            let routing = proxy.tool_routing.read().await;
            assert_eq!(
                routing.len(),
                alpha_tools.len() + beta_tools.len(),
                "Routing should have exactly one entry per tool, no duplicates"
            );
        }

        // --- Phase 4: remove_server_tools is idempotent (no-op on unknown server) ---
        let removed_unknown = proxy.remove_server_tools("gamma").await;
        assert_eq!(removed_unknown, 0, "Unknown server removal should return 0");

        // State unchanged after no-op removal
        let final_tools = proxy.get_all_tools().await;
        assert_eq!(final_tools.len(), alpha_tools.len() + beta_tools.len());
    }
}
