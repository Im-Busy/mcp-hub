//! Transport abstraction layer for mcp-hub.
//!
//! Pattern adopted from mcp-proxy-tool's TransportMode enum + dispatch.
//! Uses rmcp 1.7 for all MCP protocol handling.

pub mod stdio;
pub mod http_client;
pub mod pipe;

use crate::config::ServerConfig;
use serde::{Deserialize, Serialize};

/// Enumeration of supported transport modes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransportMode {
    /// STDIO-based transport (child process).
    Stdio,
    /// HTTP Streamable transport.
    Http,
    /// Named pipe transport.
    Pipe,
}

impl TransportMode {
    /// Determine the transport mode from a ServerConfig.
    pub fn from_config(config: &ServerConfig) -> Self {
        match config {
            ServerConfig::Stdio { .. } => TransportMode::Stdio,
            ServerConfig::Http { .. } => TransportMode::Http,
        }
    }
}

/// Transport capability flags.
#[derive(Debug, Clone, Default)]
pub struct TransportCapabilities {
    /// Whether this transport supports streaming responses.
    pub supports_streaming: bool,
    /// Whether this transport supports notifications.
    pub supports_notifications: bool,
    /// Whether this transport supports resource reading.
    pub supports_resources: bool,
    /// Whether this transport supports prompt execution.
    pub supports_prompts: bool,
}

/// A connected transport handle.
///
/// This wraps the rmcp transport and provides a unified interface
/// regardless of the underlying transport type.
pub struct TransportHandle {
    /// The transport mode.
    pub mode: TransportMode,
    /// Human-readable name for logging.
    pub name: String,
    /// Whether the connection is active.
    pub connected: bool,
    /// Transport-specific capabilities.
    pub capabilities: TransportCapabilities,
}

impl TransportHandle {
    pub fn new(mode: TransportMode, name: String) -> Self {
        Self {
            mode,
            name,
            connected: false,
            capabilities: TransportCapabilities::default(),
        }
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }
}
