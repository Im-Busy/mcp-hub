//! Host-format config renderers for MCP client integration.
//!
//! Renders HubConfig into the native format of each MCP client:
//! Claude Desktop, OpenCode, Cursor, and VS Code.

use std::collections::HashMap;

use crate::config::ServerConfig;

/// Renders MCP server configurations into host-native formats.
pub struct HostRenderer {
    /// The proxy URL (e.g., "http://localhost") without port.
    host_url: String,
    /// The port the mcp-hub proxy listens on.
    mcp_port: u16,
}

impl HostRenderer {
    /// Create a new HostRenderer.
    ///
    /// `host_url` should be the base URL without port (e.g., "http://localhost").
    /// `mcp_port` is the port the mcp-hub proxy server listens on.
    pub fn new(host_url: String, mcp_port: u16) -> Self {
        Self { host_url, mcp_port }
    }

    /// Render config in Claude Desktop format.
    ///
    /// Produces:
    /// ```json
    /// { "mcpServers": { "mcp-hub": { "command": "npx", "args": ["-y", "mcp-hub", "serve"],
    ///   "env": { "MCP_HUB_URL": "http://localhost:9090/mcp" } } } }
    /// ```
    pub fn render_claude(
        &self,
        _servers: &HashMap<String, ServerConfig>,
    ) -> serde_json::Value {
        let full_url = format!("{}:{}/mcp", self.host_url, self.mcp_port);
        serde_json::json!({
            "mcpServers": {
                "mcp-hub": {
                    "command": "npx",
                    "args": ["-y", "mcp-hub", "serve"],
                    "env": {
                        "MCP_HUB_URL": full_url
                    }
                }
            }
        })
    }

    /// Render config in OpenCode format (JSONC string).
    ///
    /// Produces a JSONC string:
    /// ```jsonc
    /// { "mcp": { "mcp-hub": { "type": "remote", "url": "http://localhost:9090/mcp", "enabled": true } } }
    /// ```
    pub fn render_opencode(
        &self,
        _servers: &HashMap<String, ServerConfig>,
    ) -> String {
        let full_url = format!("{}:{}/mcp", self.host_url, self.mcp_port);
        let value = serde_json::json!({
            "mcp": {
                "mcp-hub": {
                    "type": "remote",
                    "url": full_url,
                    "enabled": true
                }
            }
        });
        // JSONC = JSON with comments. For now, emit valid JSON (subset of JSONC).
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
    }

    /// Render config in Cursor format.
    ///
    /// Produces:
    /// ```json
    /// { "mcpServers": { "mcp-hub": { "command": "npx", "args": ["-y", "mcp-hub", "serve"] } } }
    /// ```
    pub fn render_cursor(
        &self,
        _servers: &HashMap<String, ServerConfig>,
    ) -> serde_json::Value {
        serde_json::json!({
            "mcpServers": {
                "mcp-hub": {
                    "command": "npx",
                    "args": ["-y", "mcp-hub", "serve"]
                }
            }
        })
    }

    /// Render config in VS Code format.
    ///
    /// Produces:
    /// ```json
    /// { "servers": { "mcp-hub": { "type": "stdio", "command": "npx",
    ///   "args": ["-y", "mcp-hub", "serve"] } } }
    /// ```
    pub fn render_vscode(
        &self,
        _servers: &HashMap<String, ServerConfig>,
    ) -> serde_json::Value {
        serde_json::json!({
            "servers": {
                "mcp-hub": {
                    "type": "stdio",
                    "command": "npx",
                    "args": ["-y", "mcp-hub", "serve"]
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_renderer() -> HostRenderer {
        HostRenderer::new("http://localhost".to_string(), 9090)
    }

    #[test]
    fn test_render_claude() {
        let renderer = make_renderer();
        let servers = HashMap::new();
        let result = renderer.render_claude(&servers);

        let hub = &result["mcpServers"]["mcp-hub"];
        assert_eq!(hub["command"], "npx");
        assert_eq!(hub["args"][0], "-y");
        assert_eq!(hub["args"][1], "mcp-hub");
        assert_eq!(hub["args"][2], "serve");
        assert_eq!(
            hub["env"]["MCP_HUB_URL"],
            "http://localhost:9090/mcp"
        );
    }

    #[test]
    fn test_render_opencode() {
        let renderer = make_renderer();
        let servers = HashMap::new();
        let result = renderer.render_opencode(&servers);

        assert!(result.contains("\"mcp\""));
        assert!(result.contains("\"mcp-hub\""));
        assert!(result.contains("\"type\": \"remote\""));
        assert!(result.contains("http://localhost:9090/mcp"));
        assert!(result.contains("\"enabled\": true"));
    }

    #[test]
    fn test_render_cursor() {
        let renderer = make_renderer();
        let servers = HashMap::new();
        let result = renderer.render_cursor(&servers);

        let hub = &result["mcpServers"]["mcp-hub"];
        assert_eq!(hub["command"], "npx");
        assert_eq!(hub["args"][0], "-y");
        assert_eq!(hub["args"][1], "mcp-hub");
        assert_eq!(hub["args"][2], "serve");
        // Cursor format does NOT include env
        assert!(hub.get("env").map_or(true, |e| e.is_null()));
    }

    #[test]
    fn test_render_vscode() {
        let renderer = make_renderer();
        let servers = HashMap::new();
        let result = renderer.render_vscode(&servers);

        let hub = &result["servers"]["mcp-hub"];
        assert_eq!(hub["type"], "stdio");
        assert_eq!(hub["command"], "npx");
        assert_eq!(hub["args"][0], "-y");
        assert_eq!(hub["args"][1], "mcp-hub");
        assert_eq!(hub["args"][2], "serve");
    }

    #[test]
    fn test_custom_port_and_host() {
        let renderer = HostRenderer::new("https://proxy.example.com".to_string(), 8080);
        let servers = HashMap::new();
        let result = renderer.render_claude(&servers);
        assert_eq!(
            result["mcpServers"]["mcp-hub"]["env"]["MCP_HUB_URL"],
            "https://proxy.example.com:8080/mcp"
        );
    }
}
