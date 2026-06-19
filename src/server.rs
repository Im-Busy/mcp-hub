//! HTTP/JSON-RPC server for mcp-hub.
//!
//! Pattern from MCProxy's http_server.rs: Axum-based server that exposes
//! the aggregated MCP proxy endpoint, health checks, and CORS support.

use axum::{
    extract::State,
    http::Method,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::config::HubConfig;
use crate::error::ServeError;
use crate::proxy::ProxyServer;

/// Application state shared across all request handlers.
pub struct AppState {
    /// The proxy server instance
    pub proxy: Arc<ProxyServer>,
    /// Configuration
    pub config: HubConfig,
}

/// Create the Axum router for the MCP proxy server.
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = if state.config.http_server.cors_enabled {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::DELETE])
            .allow_headers(Any)
            .allow_origin(Any)
    } else {
        CorsLayer::new()
    };

    Router::new()
        // MCP JSON-RPC endpoint
        .route("/mcp", post(handle_mcp_request))
        // Health check
        .route("/health", get(handle_health))
        // Server info
        .route("/servers", get(handle_list_servers))
        // All tools
        .route("/tools", get(handle_list_tools))
        .layer(cors)
        .with_state(state)
}

/// Start the HTTP server and listen for connections.
///
/// # Errors
/// Returns [`ServeError::Bind`] if the TCP listener cannot be created
/// (e.g., port in use). Returns [`ServeError::Server`] for accept-loop
/// errors. Returns `Ok(())` on graceful shutdown (Ctrl+C).
pub async fn serve(state: Arc<AppState>) -> Result<(), ServeError> {
    let host = state.config.http_server.host.clone();
    let port = state.config.http_server.port;
    let addr = format!("{}:{}", host, port);

    let router = create_router(state);

    info!("Starting MCP proxy server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| ServeError::Bind {
            addr: addr.clone(),
            source: e,
        })?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            info!("Shutdown signal received");
        })
        .await
        .map_err(ServeError::Server)?;

    Ok(())
}

/// Handle incoming MCP JSON-RPC requests.
async fn handle_mcp_request(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let method = body.get("method").and_then(|v| v.as_str()).unwrap_or("");

    let id = body.get("id").cloned();

    match method {
        "tools/list" => {
            let tools = state.proxy.get_all_tools().await;
            let tool_list: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                })
                .collect();

            Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": tool_list,
                }
            }))
        }

        "tools/call" => {
            let tool_name = body
                .get("params")
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let arguments = body.get("params").and_then(|p| p.get("arguments")).cloned();

            match state.proxy.call_tool(tool_name, arguments).await {
                Ok(result_text) => Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": result_text
                        }]
                    }
                })),
                Err(e) => Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32602,
                        "message": e.to_string()
                    }
                })),
            }
        }

        "initialize" => Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": true },
                    "logging": {}
                },
                "serverInfo": {
                    "name": "mcp-hub",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        })),

        "notifications/initialized" => Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {}
        })),

        _ => Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("Method not found: {}", method)
            }
        })),
    }
}

/// Health check endpoint.
async fn handle_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let servers = state.proxy.get_server_infos().await;
    let connected = servers.iter().filter(|s| s.connected).count();

    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "servers_total": servers.len(),
        "servers_connected": connected,
        "tools_total": state.proxy.get_all_tools().await.len(),
    }))
}

/// List all connected servers.
async fn handle_list_servers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let servers = state.proxy.get_server_infos().await;
    let server_list: Vec<Value> = servers
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "connected": s.connected,
                "tool_count": s.tool_count,
                "last_refresh": s.last_refresh.map(|t| t.to_rfc3339()),
            })
        })
        .collect();

    Json(json!({ "servers": server_list }))
}

/// List all aggregated tools.
async fn handle_list_tools(State(state): State<Arc<AppState>>) -> Json<Value> {
    let tools = state.proxy.get_all_tools().await;
    let tool_list: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "server": t.server_name,
                "original_name": t.original_name,
                "description": t.description,
            })
        })
        .collect();

    Json(json!({ "tools": tool_list }))
}
