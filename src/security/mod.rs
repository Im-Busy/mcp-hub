//! Security module (Phase 2).
//!
//! Pattern from tool-cli: AES-256-GCM encrypted credentials,
//! secure key management with auto-generation, and zeroize on drop.

pub mod encryption;
pub mod auth;
pub mod rate_limit;
