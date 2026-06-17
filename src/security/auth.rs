//! Auth module (Phase 2).
//!
//! Pattern from pluggedin-mcp-proxy: lazy authentication —
//! tool discovery endpoints are public, tool calls require auth.

/// Placeholder for auth module.
pub fn validate_token(_token: &str) -> Result<(), crate::error::HubError> {
    Ok(())
}
