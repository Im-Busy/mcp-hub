//! Built-in time tool — always available regardless of upstream server connectivity.
//!
//! Uses chrono (already a Cargo.toml dependency) to provide:
//! - get_current_time: Current time in human-readable format
//! - get_current_time_iso: ISO 8601 format
//!
//! This replaces the external @guanxiong/mcp-server-time (1★, solo dev, stale).

use chrono::{DateTime, Utc};

/// Get current time in human-readable format.
pub fn get_current_time() -> String {
    let now: DateTime<Utc> = Utc::now();
    format!(
        "{} (Unix: {}, ISO: {})",
        now.format("%Y-%m-%d %H:%M:%S %Z"),
        now.timestamp(),
        now.to_rfc3339(),
    )
}

/// Get current time in ISO 8601 format.
pub fn get_current_time_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Get current Unix timestamp.
pub fn get_unix_timestamp() -> i64 {
    Utc::now().timestamp()
}
