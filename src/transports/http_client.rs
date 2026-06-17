//! HTTP Streamable transport — connects to MCP servers via HTTP.
//!
//! Pattern adopted from MCProxy: BearerAuthClient decorator for transparent
//! auth token injection, StreamableHttpClientTransport for rmcp integration.
//!
//! NOTE: BearerAuthClient currently disabled due to rmcp 1.7 API changes.
//! Auth tokens are passed via ServerConfig and will be re-enabled in Phase 2.1.

use crate::config::ServerConfig;
use crate::error::{HubError, HubResult};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransport;
use std::sync::Arc;
use tracing::info;

/// Connect to an HTTP Streamable MCP server via rmcp.
pub async fn connect_http(
    config: &ServerConfig,
    server_name: &str,
) -> HubResult<RunningService<RoleClient, ()>> {
    let url = match config {
        ServerConfig::Http { url, .. } => url,
        _ => return Err(HubError::Transport(format!(
            "Expected HTTP config for server '{}'", server_name
        ))),
    };

    info!(server = %server_name, url = %url, "Connecting to HTTP MCP server");

    let uri: Arc<str> = Arc::from(url.as_str());
    let transport = StreamableHttpClientTransport::from_uri(uri);
    let handler = ();

    handler.serve(transport).await.map_err(|e| HubError::Connection {
        server: server_name.to_string(),
        message: format!("rmcp HTTP serve failed: {}", e),
    })
}
