//! HTTP Streamable transport — connects to MCP servers via HTTP.
//!
//! Supports Bearer token auth via BearerAuthClient wrapping reqwest.

use crate::config::ServerConfig;
use crate::error::{HubError, HubResult};
use rmcp::{
    model::ClientJsonRpcMessage,
    service::{RoleClient, RunningService, ServiceExt},
    transport::streamable_http_client::{
        StreamableHttpClient, StreamableHttpClientTransport,
        StreamableHttpClientTransportConfig, StreamableHttpError,
        StreamableHttpPostResponse,
    },
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

/// BearerAuthClient wraps reqwest with automatic Bearer token injection.
#[derive(Clone)]
pub struct BearerAuthClient {
    inner: reqwest::Client,
    token: String,
}

impl BearerAuthClient {
    pub fn new(inner: reqwest::Client, token: String) -> Self {
        Self { inner, token }
    }
}

impl StreamableHttpClient for BearerAuthClient {
    type Error = <reqwest::Client as StreamableHttpClient>::Error;

    async fn post_message(
        &self, uri: Arc<str>, message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>, _auth_header: Option<String>,
        custom_headers: HashMap<http::HeaderName, http::HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.inner.post_message(uri, message, session_id,
            Some(format!("Bearer {}", self.token)), custom_headers).await
    }

    async fn delete_session(
        &self, uri: Arc<str>, session_id: Arc<str>,
        _auth_header: Option<String>,
        custom_headers: HashMap<http::HeaderName, http::HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        self.inner.delete_session(uri, session_id,
            Some(format!("Bearer {}", self.token)), custom_headers).await
    }

    async fn get_stream(
        &self, uri: Arc<str>, session_id: Arc<str>,
        last_event_id: Option<String>, _auth_header: Option<String>,
        custom_headers: HashMap<http::HeaderName, http::HeaderValue>,
    ) -> Result<
        futures_util::stream::BoxStream<'static, Result<sse_stream::Sse, sse_stream::Error>>,
        StreamableHttpError<Self::Error>,
    > {
        self.inner.get_stream(uri, session_id, last_event_id,
            Some(format!("Bearer {}", self.token)), custom_headers).await
    }
}

/// Connect to an HTTP Streamable MCP server, with optional Bearer auth.
pub async fn connect_http(
    config: &ServerConfig,
    server_name: &str,
) -> HubResult<RunningService<RoleClient, ()>> {
    let (url, auth_token) = match config {
        ServerConfig::Http { url, authorization_token, .. } => (url, authorization_token),
        _ => return Err(HubError::Transport(format!(
            "Expected HTTP config for server '{}'", server_name
        ))),
    };

    info!(server = %server_name, url = %url, has_auth = auth_token.is_some(), "Connecting to HTTP MCP server");

    let uri: Arc<str> = Arc::from(url.as_str());
    let handler = ();

    let result = if let Some(token) = auth_token {
        if token.is_empty() {
            handler.serve(StreamableHttpClientTransport::from_uri(uri)).await
        } else {
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build().map_err(|e| HubError::Connection {
                    server: server_name.to_string(),
                    message: format!("HTTP client build failed: {}", e),
                })?;
            let auth = BearerAuthClient::new(http_client, token.clone());
            let config = StreamableHttpClientTransportConfig::with_uri(uri);
            handler.serve(StreamableHttpClientTransport::with_client(auth, config)).await
        }
    } else {
        handler.serve(StreamableHttpClientTransport::from_uri(uri)).await
    };

    result.map_err(|e| HubError::Connection {
        server: server_name.to_string(),
        message: format!("rmcp HTTP serve failed: {}", e),
    })
}
