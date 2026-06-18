//! Configuration structures for mcp-hub.
//!
//! Pattern adopted from MCProxy's config.rs — supports both Stdio and HTTP
//! upstream servers, plus HTTP server configuration for the proxy endpoint.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Server priority layer for connection ordering and reliability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerLayer {
    /// Critical: connect first, retry 3x with backoff, health monitored.
    Critical,
    /// Standard: connect after L1, retry 1x, default.
    Standard,
    /// Optional: connect async in background, no retry, best-effort.
    Optional,
}

fn default_layer() -> ServerLayer {
    ServerLayer::Standard
}

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HubConfig {
    /// Upstream MCP servers to aggregate.
    #[serde(default)]
    pub mcp_servers: HashMap<String, ServerConfig>,

    /// HTTP server configuration for exposing the aggregated proxy.
    #[serde(default)]
    pub http_server: HttpServerConfig,

    /// Middleware configuration.
    #[serde(default)]
    pub middleware: MiddlewareConfig,

    /// Package registry configuration.
    #[serde(default)]
    pub registry: RegistryConfig,

    /// Host integration configuration.
    #[serde(default)]
    pub hosts: HostsConfig,

    /// Built-in tools configuration.
    #[serde(default)]
    pub tools: ToolsConfig,
}

/// Configuration for a single upstream MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerConfig {
    /// STDIO-based MCP server (spawned as child process).
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default = "default_layer")]
        layer: ServerLayer,
    },

    /// HTTP Streamable MCP server.
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default)]
        authorization_token: Option<String>,
        #[serde(default = "default_layer")]
        layer: ServerLayer,
    },
}

/// HTTP server configuration for the proxy endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    /// Host to bind to.
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Whether CORS is enabled.
    #[serde(default = "default_true")]
    pub cors_enabled: bool,

    /// Allowed CORS origins.
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,

    /// Shutdown timeout in seconds.
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    9090
}

fn default_true() -> bool {
    true
}

fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_shutdown_timeout() -> u64 {
    10
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            cors_enabled: true,
            cors_origins: default_cors_origins(),
            shutdown_timeout_secs: default_shutdown_timeout(),
        }
    }
}

/// Middleware configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MiddlewareConfig {
    /// Proxy-level middleware (operates on aggregated results).
    #[serde(default)]
    pub proxy: Vec<MiddlewareSpec>,

    /// Client-level middleware (operates per-server).
    #[serde(default)]
    pub client: ClientMiddlewareConfig,
}

/// Client middleware configuration with per-server overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientMiddlewareConfig {
    /// Default middleware for all servers.
    #[serde(default)]
    pub default: Vec<MiddlewareSpec>,

    /// Per-server overrides (completely replaces default for that server).
    #[serde(default)]
    pub servers: HashMap<String, Vec<MiddlewareSpec>>,
}

/// Specification for a single middleware instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareSpec {
    /// Middleware type name.
    pub r#type: String,

    /// Whether this middleware is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Middleware-specific configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Package registry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry base URL.
    #[serde(default = "default_registry_url")]
    pub url: String,

    /// Authentication token for the registry.
    pub token: Option<String>,
}

fn default_registry_url() -> String {
    "https://registry.mcp-hub.dev".to_string()
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            url: default_registry_url(),
            token: None,
        }
    }
}

/// Host integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostsConfig {
    /// Whether to enable automatic host integration.
    #[serde(default)]
    pub auto_register: bool,

    /// Configured hosts.
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// Built-in tools configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Whether clipboard/memory tools are enabled.
    #[serde(default = "default_true")]
    pub clipboard_enabled: bool,

    /// Whether tool search middleware is enabled.
    #[serde(default = "default_true")]
    pub search_enabled: bool,

    /// Maximum tools to expose initially (before search is needed).
    #[serde(default = "default_max_tools")]
    pub max_tools_limit: usize,

    /// Search relevance threshold (0.0 - 1.0).
    #[serde(default = "default_search_threshold")]
    pub search_threshold: f32,
}

fn default_max_tools() -> usize {
    50
}

fn default_search_threshold() -> f32 {
    0.1
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            clipboard_enabled: true,
            search_enabled: true,
            max_tools_limit: default_max_tools(),
            search_threshold: default_search_threshold(),
        }
    }
}

impl HubConfig {
    /// Load configuration from a JSON file.
    pub fn load(path: &PathBuf) -> crate::error::HubResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::HubError::Config(format!("Failed to read config file: {}", e)))?;

        let config: HubConfig = serde_json::from_str(&content)
            .map_err(|e| crate::error::HubError::Config(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// Get middleware specs for a specific server, resolving overrides.
    pub fn get_client_middleware_for_server(&self, server_name: &str) -> Vec<MiddlewareSpec> {
        self.middleware
            .client
            .servers
            .get(server_name)
            .cloned()
            .unwrap_or_else(|| self.middleware.client.default.clone())
    }
}
