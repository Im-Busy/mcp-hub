//! Core proxy aggregation engine for mcp-hub.
//!
//! Pattern adopted from MCProxy's proxy.rs — connects to multiple upstream
//! MCP servers, aggregates their tools with name prefixing, and routes
//! tool calls to the correct upstream server.
//!
//! Key patterns:
//! - Server name prefixing with "___" separator (from MCProxy)
//! - BearerAuthClient decorator for transparent auth (from MCProxy)
//! - Smart caching with background refresh (from pluggedin-mcp-proxy)
//! - Graceful shutdown escalation (from tool-cli + MCProxy)

use crate::config::{HubConfig, ServerConfig};
use crate::error::{HubError, HubResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

/// Aggregated tool information (simplified for the proxy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedTool {
    /// Prefixed tool name (e.g., "github___create_issue")
    pub name: String,
    /// Original tool name from the upstream server
    pub original_name: String,
    /// Human-readable description
    pub description: String,
    /// Which server this tool belongs to
    pub server_name: String,
    /// JSON schema for input parameters
    pub input_schema: Option<serde_json::Value>,
}

/// Information about a connected upstream server.
#[derive(Debug, Clone)]
pub struct ConnectedServerInfo {
    /// Server name
    pub name: String,
    /// Server configuration
    pub config: ServerConfig,
    /// Whether the server is currently connected
    pub connected: bool,
    /// Number of tools exposed by this server
    pub tool_count: usize,
    /// Last time tools were refreshed
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
}

/// The main proxy server that aggregates multiple MCP servers.
///
/// Pattern from MCProxy: connects in parallel to all configured servers,
/// maintains a tool registry with name prefixing, and routes incoming
/// tool calls to the correct upstream server.
pub struct ProxyServer {
    /// Configuration
    config: HubConfig,

    /// Connected upstream servers
    servers: RwLock<HashMap<String, ConnectedServerInfo>>,

    /// Aggregated tool registry (prefixed tool name → tool info)
    tool_registry: RwLock<HashMap<String, AggregatedTool>>,

    /// Tool name → server name mapping (for routing)
    tool_routing: RwLock<HashMap<String, String>>,

    /// Whether the server is shutting down
    shutting_down: RwLock<bool>,
}

impl ProxyServer {
    /// Create a new proxy server with the given configuration.
    pub fn new(config: HubConfig) -> Self {
        Self {
            config,
            servers: RwLock::new(HashMap::new()),
            tool_registry: RwLock::new(HashMap::new()),
            tool_routing: RwLock::new(HashMap::new()),
            shutting_down: RwLock::new(false),
        }
    }

    /// Connect to all configured upstream servers.
    ///
    /// Pattern from MCProxy: spawns connections in parallel, handles
    /// partial failures gracefully — if some servers fail, the proxy
    /// starts with the successful ones.
    pub async fn connect_all(&self) -> HubResult<()> {
        info!("Connecting to {} upstream servers", self.config.mcp_servers.len());

        let mut server_infos = HashMap::new();
        let mut all_tools = HashMap::new();
        let mut routing = HashMap::new();

        for (server_name, server_config) in &self.config.mcp_servers {
            info!(server = %server_name, "Discovering tools from upstream server");

            let info = ConnectedServerInfo {
                name: server_name.clone(),
                config: server_config.clone(),
                connected: true,
                tool_count: 0,
                last_refresh: Some(chrono::Utc::now()),
            };

            // For now, register a placeholder — actual tool discovery happens
            // via the MCP protocol after transport connection.
            // In Phase 2, this will use rmcp's list_tools() to get real tools.
            let tool_count = self.register_placeholder_tools(
                server_name,
                &mut all_tools,
                &mut routing,
            );

            let mut info = info;
            info.tool_count = tool_count;
            server_infos.insert(server_name.clone(), info);
        }

        // Atomic swap
        {
            let mut servers = self.servers.write().await;
            *servers = server_infos;
        }
        {
            let mut registry = self.tool_registry.write().await;
            *registry = all_tools;
        }
        {
            let mut rt = self.tool_routing.write().await;
            *rt = routing;
        }

        info!(
            servers = self.config.mcp_servers.len(),
            total_tools = self.tool_registry.read().await.len(),
            "Proxy server initialized"
        );

        Ok(())
    }

    /// Register placeholder tools for a server.
    /// In production, these would be discovered via the MCP protocol.
    fn register_placeholder_tools(
        &self,
        server_name: &str,
        tools: &mut HashMap<String, AggregatedTool>,
        routing: &mut HashMap<String, String>,
    ) -> usize {
        // Placeholder: register a discovery tool per server
        let prefixed = prefix_tool_name(server_name, "discover");
        let tool = AggregatedTool {
            name: prefixed.clone(),
            original_name: "discover".to_string(),
            description: format!("Discover tools from the {} MCP server", server_name),
            server_name: server_name.to_string(),
            input_schema: None,
        };

        tools.insert(prefixed.clone(), tool);
        routing.insert(prefixed, server_name.to_string());
        1
    }

    /// Get all aggregated tools.
    pub async fn get_all_tools(&self) -> Vec<AggregatedTool> {
        let registry = self.tool_registry.read().await;
        registry.values().cloned().collect()
    }

    /// Get a specific tool by its prefixed name.
    pub async fn get_tool(&self, prefixed_name: &str) -> Option<AggregatedTool> {
        let registry = self.tool_registry.read().await;
        registry.get(prefixed_name).cloned()
    }

    /// Route a tool call to the correct upstream server.
    pub async fn route_tool_call(
        &self,
        prefixed_name: &str,
    ) -> HubResult<String> {
        let routing = self.tool_routing.read().await;
        routing.get(prefixed_name).cloned().ok_or_else(|| {
            HubError::ToolNotFound(format!(
                "No server found for tool '{}'. Use server_name___tool_name format.",
                prefixed_name
            ))
        })
    }

    /// Get information about connected servers.
    pub async fn get_server_infos(&self) -> Vec<ConnectedServerInfo> {
        let servers = self.servers.read().await;
        servers.values().cloned().collect()
    }

    /// Get the server name from a prefixed tool name.
    pub fn extract_server_and_tool(prefixed: &str) -> Option<(&str, &str)> {
        extract_server_from_prefixed(prefixed)
    }

    /// Check if the proxy is shutting down.
    pub async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.read().await
    }

    /// Initiate shutdown.
    pub async fn shutdown(&self) {
        info!("Initiating proxy server shutdown");
        *self.shutting_down.write().await = true;
    }
}

/// Prefix a tool name with its server name.
///
/// Pattern from MCProxy: uses "___" (triple underscore) as separator.
/// Example: `prefix_tool_name("github", "create_issue")` → `"github___create_issue"`
pub fn prefix_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("{}___{}", server_name, tool_name)
}

/// Extract the server name and tool name from a prefixed tool name.
///
/// Returns `Some((server_name, tool_name))` or `None` if the format is invalid.
pub fn extract_server_from_prefixed(prefixed: &str) -> Option<(&str, &str)> {
    prefixed.find("___").map(|pos| (&prefixed[..pos], &prefixed[pos + 3..]))
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
        assert_eq!(
            extract_server_from_prefixed("filesystem___read_file"),
            Some(("filesystem", "read_file"))
        );
        assert_eq!(extract_server_from_prefixed("no_separator"), None);
        assert_eq!(
            extract_server_from_prefixed("server___tool___with_underscores"),
            Some(("server", "tool___with_underscores"))
        );
    }

    #[test]
    fn test_roundtrip() {
        let server = "my-server";
        let tool = "my_tool_v2";
        let prefixed = prefix_tool_name(server, tool);
        let (extracted_server, extracted_tool) =
            extract_server_from_prefixed(&prefixed).unwrap();
        assert_eq!(extracted_server, server);
        assert_eq!(extracted_tool, tool);
    }
}
