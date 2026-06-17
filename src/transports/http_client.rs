//! HTTP Streamable transport — connects to MCP servers via HTTP.
//!
//! Pattern adopted from MCProxy's BearerAuthClient decorator:
//! - Wraps reqwest client with automatic auth token injection
//! - Supports streamable HTTP endpoints

use crate::config::ServerConfig;
use crate::error::{HubError, HubResult};
use tracing::{info, debug};

/// Configuration for HTTP transport connection.
#[derive(Debug, Clone)]
pub struct HttpTransportConfig {
    /// Base URL for the MCP endpoint.
    pub url: String,
    /// Optional bearer token for authentication.
    pub auth_token: Option<String>,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl HttpTransportConfig {
    /// Create from a ServerConfig::Http variant.
    pub fn from_config(config: &ServerConfig, server_name: &str) -> HubResult<Self> {
        match config {
            ServerConfig::Http {
                url,
                authorization_token,
            } => Ok(Self {
                url: url.clone(),
                auth_token: authorization_token.clone(),
                timeout_secs: 30,
            }),
            _ => Err(HubError::Transport(format!(
                "Expected HTTP config for server '{}'",
                server_name
            ))),
        }
    }
}

impl Default for HttpTransportConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080/mcp".to_string(),
            auth_token: None,
            timeout_secs: 30,
        }
    }
}

/// Connect to an HTTP Streamable MCP server.
///
/// Returns a tuple of (reqwest::Client, config) for use with
/// rmcp's streamable HTTP client transport.
pub fn connect_http(
    config: &HttpTransportConfig,
    server_name: &str,
) -> HubResult<(reqwest::Client, String)> {
    info!(
        server = %server_name,
        url = %config.url,
        "Connecting to HTTP MCP server"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| HubError::Connection {
            server: server_name.to_string(),
            message: format!("Failed to build HTTP client: {}", e),
        })?;

    debug!(
        server = %server_name,
        has_auth = config.auth_token.is_some(),
        "HTTP client created"
    );

    Ok((client, config.url.clone()))
}
