//! Structured error types for mcp-hub.
//!
//! Pattern adopted from MCProxy's ProxyError and tool-cli's ToolError.

use thiserror::Error;

/// Primary error type for all mcp-hub operations.
#[derive(Error, Debug)]
pub enum HubError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Server connection error for '{server}': {message}")]
    Connection { server: String, message: String },

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Middleware error: {0}")]
    Middleware(String),

    #[error("Security violation: {rule} on server '{server}'")]
    SecurityViolation { rule: String, server: String },

    #[error("Invalid tool name format: {0}. Expected format: server___tool_name")]
    InvalidToolFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("MCP protocol error: {0}")]
    Mcp(String),

    #[error("MCP service error: {0}")]
    McpService(String),

    #[error("Process spawn error: {0}")]
    ProcessSpawn(String),

    #[error("Package error: {0}")]
    Package(String),

    #[error("Host integration error: {0}")]
    Host(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for HubError {
    fn from(e: anyhow::Error) -> Self {
        HubError::Other(format!("{:#}", e))
    }
}

/// Convenience result type alias.
pub type HubResult<T> = Result<T, HubError>;

/// Errors that can occur during server startup and operation.
///
/// Separates bind errors (potentially retryable) from runtime errors (fatal).
#[derive(Error, Debug)]
pub enum ServeError {
    /// Failed to bind the TCP listener (e.g., AddrInUse, PermissionDenied).
    #[error("Failed to bind to {addr}: {source}")]
    Bind {
        /// Address that was attempted
        addr: String,
        /// Underlying IO error
        #[source]
        source: std::io::Error,
    },
    /// Non-bind server error during accept loop or graceful shutdown.
    #[error("Server error: {0}")]
    Server(#[source] std::io::Error),
}
