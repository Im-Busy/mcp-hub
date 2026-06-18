//! mcp-hub — Unified MCP Management Platform
//!
//! Combines proxy aggregation (from MCProxy), package management (from tool-cli),
//! inspection/debugging (from MCP Inspector), and memory tools (from pluggedin-mcp-proxy)
//! into a single Rust binary.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                    MCP Clients (Cursor, Claude, etc.) │
//! └────────────────────────┬─────────────────────────────┘
//!                          │ JSON-RPC via STDIO or HTTP
//!                          ▼
//! ┌──────────────────────────────────────────────────────┐
//! │                    mcp-hub Server                     │
//! │  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
//! │  │ Proxy Engine │  │  Middleware  │  │  HTTP API  │ │
//! │  │ (aggregate,  │◄─┤  (logging,   │◄─┤  (Axum,    │ │
//! │  │  prefix,     │  │   security,  │  │   CORS,    │ │
//! │  │  route)      │  │   search)    │  │   /health) │ │
//! │  └──────┬───────┘  └──────────────┘  └────────────┘ │
//! │         │                                             │
//! │  ┌──────┴──────────────────────────────────────┐     │
//! │  │           Transport Layer                     │     │
//! │  │  STDIO   │   HTTP    │   Named Pipe          │     │
//! │  │  (child  │  (reqwest │   (Unix socket/       │     │
//! │  │  process)│   client) │    Win pipe)           │     │
//! │  └──────┬───┴─────┬─────┴────────┬──────────────┘     │
//! └─────────┼─────────┼──────────────┼────────────────────┘
//!           │         │              │
//!           ▼         ▼              ▼
//!     ┌─────────┐ ┌─────────┐ ┌───────────┐
//!     │ MCP Svr │ │ MCP Svr │ │ MCP Svr   │
//!     │ (stdio) │ │  (http) │ │  (pipe)   │
//!     └─────────┘ └─────────┘ └───────────┘
//! ```
//!
//! ## Key Patterns
//!
//! - **Dual Middleware**: ClientMiddleware (per-server) + ProxyMiddleware (cross-server)
//!   — from MCProxy
//! - **Transport Enum Dispatch**: TransportMode enum with per-mode connection logic
//!   — from mcp-proxy-tool
//! - **Server Name Prefixing**: `"server___tool"` format for collision-free aggregation
//!   — from MCProxy
//! - **Smart Caching**: Return cached results instantly, refresh in background
//!   — from pluggedin-mcp-proxy
//! - **AES-256-GCM Encrypted Config**: Credential envelope with nonce+auth_tag+key_id
//!   — from tool-cli

pub mod cli;
pub mod config;
pub mod error;
pub mod middleware;
pub mod proxy;
pub mod server;
pub mod transports;

// Stub modules (Phase 2+ implementation)
pub mod package;
pub mod hosts;
pub mod security;
pub mod tools;

pub use config::{HubConfig, ServerLayer};
pub use error::{HubError, HubResult};
pub use proxy::ProxyServer;
