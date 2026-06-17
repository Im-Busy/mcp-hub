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
    Connection {
        server: String,
        message: String,
    },

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
