//! mcp-hub — Unified MCP Management Platform
//!
//! Main entry point. Parses CLI arguments, loads configuration,
//! initializes the proxy server, and starts the HTTP listener.

use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

use mcp_hub::cli::{build_cli, Commands};
use mcp_hub::config::HubConfig;
use mcp_hub::proxy::ProxyServer;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let cli = build_cli();

    if cli.verbose {
        // Verbose logging is handled by the env filter
    }

    match cli.command {
        Some(Commands::Serve {
            config,
            host,
            port,
            no_cors,
        }) => {
            let config_path = config.unwrap_or(cli.config);
            info!(path = %config_path.display(), "Loading configuration");

            let mut hub_config = match HubConfig::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to load config: {}", e);
                    std::process::exit(1);
                }
            };

            // Apply CLI overrides
            if let Some(h) = host {
                hub_config.http_server.host = h;
            }
            if let Some(p) = port {
                hub_config.http_server.port = p;
            }
            if no_cors {
                hub_config.http_server.cors_enabled = false;
            }

            run_serve(hub_config).await;
        }

        Some(Commands::Status { config }) => {
            let config_path = config.unwrap_or(cli.config);
            info!(path = %config_path.display(), "Loading configuration");

            let hub_config = match HubConfig::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to load config: {}", e);
                    std::process::exit(1);
                }
            };

            let proxy = Arc::new(ProxyServer::new(hub_config));
            if let Err(e) = proxy.connect_all().await {
                error!("Failed to connect: {}", e);
                std::process::exit(1);
            }

            let servers = proxy.get_server_infos().await;
            let tools = proxy.get_all_tools().await;

            println!("=== mcp-hub Status ===");
            println!("Servers: {}", servers.len());
            for s in &servers {
                let status = if s.connected { "✓" } else { "✗" };
                println!("  {} {} ({} tools)", status, s.name, s.tool_count);
            }
            println!("Total aggregated tools: {}", tools.len());
        }

        Some(Commands::Validate { config }) => {
            match HubConfig::load(&config) {
                Ok(_) => {
                    println!("✓ Configuration is valid: {}", config.display());
                }
                Err(e) => {
                    error!("✗ Invalid configuration: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Some(Commands::Inspect { target, args }) => {
            println!("=== MCP Inspector ===");
            println!("Target: {}", target);
            if !args.is_empty() {
                println!("Arguments: {}", args.join(" "));
            }
            println!("(Interactive inspection will be available in Phase 3)");
        }

        Some(Commands::Install { package, registry }) => {
            println!("Installing: {}", package);
            if let Some(r) = registry {
                println!("  Registry: {}", r);
            }
            println!("(Package management will be available in Phase 2)");
        }

        Some(Commands::Publish { path, registry }) => {
            println!("Publishing: {}", path.display());
            if let Some(r) = registry {
                println!("  Registry: {}", r);
            }
            println!("(Publishing will be available in Phase 2)");
        }

        Some(Commands::Host { host, tool, remove, list }) => {
            println!("Host: {}", host);
            if let Some(t) = tool {
                if remove {
                    println!("  Removing tool: {}", t);
                } else {
                    println!("  Adding tool: {}", t);
                }
            }
            if list {
                println!("  (Listing tools)");
            }
            println!("(Host integration will be available in Phase 2)");
        }

        None => {
            // Default: serve with the config file
            let hub_config = match HubConfig::load(&cli.config) {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to load config: {}", e);
                    std::process::exit(1);
                }
            };
            run_serve(hub_config).await;
        }
    }
}

/// Run the proxy server with the given configuration.
async fn run_serve(config: HubConfig) {
    info!("Starting mcp-hub v{}", env!("CARGO_PKG_VERSION"));

    let proxy = Arc::new(ProxyServer::new(config.clone()));

    // Connect to upstream servers
    if let Err(e) = proxy.connect_all().await {
        error!("Failed to connect to upstream servers: {}", e);
        // Continue anyway — the proxy can still serve what's connected
    }

    let state = Arc::new(mcp_hub::server::AppState {
        proxy,
        config: config.clone(),
    });

    let host = config.http_server.host.clone();
    let port = config.http_server.port;

    info!("Proxy server listening on http://{}:{}/mcp", host, port);
    info!("Health check: http://{}:{}/health", host, port);

    if let Err(e) = mcp_hub::server::serve(state).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
