//! Package management module (Phase 2).
//!
//! Pattern from tool-cli: install, publish, and manage MCP servers.
//! Uses the MCPBX manifest format for package definitions.

pub mod manifest;

/// Placeholder for package installation.
pub fn install(_package: &str, _registry: Option<&str>) -> crate::error::HubResult<()> {
    Ok(())
}

/// Placeholder for package publishing.
pub fn publish(_path: &std::path::Path, _registry: Option<&str>) -> crate::error::HubResult<()> {
    Ok(())
}
