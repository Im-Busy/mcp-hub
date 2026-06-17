//! Named Pipe transport — connects to MCP servers via named pipes.
//!
//! Pattern adopted from mcp-proxy-tool's cross-platform named pipe abstraction.
//! - Unix: Unix domain sockets
//! - Windows: Windows named pipes

use crate::error::{HubError, HubResult};
use tracing::info;

/// Named pipe configuration.
#[derive(Debug, Clone)]
pub struct PipeConfig {
    /// Path to the named pipe.
    /// On Unix: socket path (e.g., /tmp/mcp_server.sock)
    /// On Windows: pipe name (e.g., \\.\pipe\mcp_server or short form: mcp_server)
    pub path: String,
}

impl PipeConfig {
    pub fn new(path: String) -> Self {
        // Auto-normalize Windows pipe paths
        #[cfg(windows)]
        let path = {
            if !path.starts_with("\\\\.\\pipe\\") && !path.contains('\\') {
                format!("\\\\.\\pipe\\{}", path)
            } else {
                path
            }
        };

        Self { path }
    }
}

/// Connect to a named pipe MCP server.
///
/// Note: Currently a placeholder. rmcp does not natively support named pipe
/// transport, so this would need custom implementation. The pattern from
/// mcp-proxy-tool shows how to do this with raw tokio I/O.
pub fn connect_pipe(config: &PipeConfig, server_name: &str) -> HubResult<()> {
    info!(
        server = %server_name,
        path = %config.path,
        "Connecting to named pipe MCP server"
    );

    // Named pipe transport requires custom implementation since rmcp
    // doesn't support it natively. For now, this is a documented limitation.

    Err(HubError::Transport(format!(
        "Named pipe transport is not yet implemented for server '{}'. \
         The pattern from mcp-proxy-tool shows how to implement this \
         using raw tokio I/O with Unix sockets or Windows named pipes. \
         See: https://github.com/awakecoding/mcp-proxy-tool",
        server_name
    )))
}
