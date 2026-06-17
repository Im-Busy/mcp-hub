//! Host integration module (Phase 2).
//!
//! Pattern from tool-cli: register MCP servers with MCP clients
//! (Claude Desktop, Cursor, VS Code, OpenCode, etc.).

/// Placeholder for host integration.
pub mod atomic_write;

/// Register a tool with a host.
pub fn register(_host: &str, _tool: &str) -> crate::error::HubResult<()> {
    Ok(())
}

/// Remove a tool from a host.
pub fn unregister(_host: &str, _tool: &str) -> crate::error::HubResult<()> {
    Ok(())
}
