//! Built-in tools module (Phase 2).
//!
//! Pattern from pluggedin-mcp-proxy: static built-in tools that are
//! always available regardless of downstream server connectivity.
//! Includes clipboard/memory system for agent persistence,
//! and time tools (replacing external time server dependency).

pub mod clipboard;
pub mod time;
