//! CLI interface for mcp-hub.
//!
//! Pattern from tool-cli: uses clap 4.5 with derive macros for rich help.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// mcp-hub — Unified MCP management platform.
///
/// Combines proxy aggregation, package management, inspection,
/// and memory tools for MCP servers.
#[derive(Parser)]
#[command(name = "mcp-hub")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Unified MCP management platform", long_about = None)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "mcp-hub.json")]
    pub config: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the proxy server (aggregate upstream MCP servers)
    Serve {
        /// Config file path
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Override HTTP host
        #[arg(long)]
        host: Option<String>,

        /// Override HTTP port
        #[arg(long)]
        port: Option<u16>,

        /// Disable CORS
        #[arg(long)]
        no_cors: bool,
    },

    /// List connected servers and their tools
    Status {
        /// Config file path
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Validate a configuration file
    Validate {
        /// Config file path
        #[arg(short, long)]
        config: PathBuf,
    },

    /// Inspect an MCP server interactively
    Inspect {
        /// The MCP server command or URL to inspect
        target: String,

        /// Arguments for the server command
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Install an MCP server from registry
    Install {
        /// Package reference (e.g., library/bash)
        package: String,

        /// Registry URL override
        #[arg(long)]
        registry: Option<String>,
    },

    /// Publish an MCP server to the registry
    Publish {
        /// Path to the MCP server directory
        path: PathBuf,

        /// Registry URL override
        #[arg(long)]
        registry: Option<String>,
    },

    /// Add a tool to a host's MCP configuration
    Host {
        /// Host name (claude-desktop, cursor, vscode, etc.)
        host: String,

        /// Tool to add
        tool: Option<String>,

        /// Remove the tool from the host instead
        #[arg(long)]
        remove: bool,

        /// List configured tools for the host
        #[arg(long)]
        list: bool,
    },
}

/// Build the CLI parser.
pub fn build_cli() -> Cli {
    Cli::parse()
}
