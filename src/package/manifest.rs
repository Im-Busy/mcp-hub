//! MCPBX manifest format (Phase 2).
//!
//! Pattern from tool-cli: superset of MCPB with additional metadata
//! for transport bridging, OAuth, and system configuration.

use serde::{Deserialize, Serialize};

/// MCPBX manifest — describes an MCP server package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpbxManifest {
    /// Package name (e.g., "library/bash").
    pub name: String,

    /// Package version (semver).
    pub version: String,

    /// Human-readable description.
    pub description: String,

    /// Transport type.
    pub transport: TransportType,

    /// Entry point (command for stdio, URL for HTTP).
    pub entry: String,

    /// Environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

    /// OAuth configuration (if applicable).
    pub oauth: Option<OAuthConfig>,

    /// System configuration for resource allocation.
    #[serde(default)]
    pub system_config: SystemConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    Http,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemConfig {
    pub needs_port: bool,
    pub needs_data_dir: bool,
    pub needs_temp_dir: bool,
}
