//! Integration tests for mcp-hub — end-to-end transport verification.
//!
//! These tests exercise real MCP transport connections: spawn a real MCP server,
//! connect via rmcp stdio transport, discover tools, and call them.

use mcp_hub::config::{HubConfig, ServerConfig};
use mcp_hub::transports::stdio;
use mcp_hub::ProxyServer;
use std::collections::HashMap;

/// Raw transport test: connect to time server via stdio directly (no proxy layer).
/// This isolates transport-level issues from proxy aggregation issues.
#[tokio::test]
async fn test_raw_stdio_connection() {
    if !command_exists("npx") {
        eprintln!("SKIP: npx not available on PATH");
        return;
    }

    let config = ServerConfig::Stdio {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@guanxiong/mcp-server-time@1.0.0".to_string()],
        env: HashMap::new(),
        layer: mcp_hub::config::ServerLayer::Critical,
    };

    eprintln!("Attempting raw stdio connection...");
    match stdio::connect_stdio(&config, "test-time").await {
        Ok((_svc, _child)) => {
            eprintln!("✅ Raw stdio connection succeeded!");
        }
        Err(e) => {
            // Don't panic — print the error for debugging
            eprintln!("❌ Raw stdio connection failed: {:?}", e);
            panic!("Raw stdio connection failed: {}", e);
        }
    }
}

/// Check if a command exists on the system PATH.
fn command_exists(cmd: &str) -> bool {
    if cfg!(windows) {
        std::process::Command::new("where")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Helper: build a HubConfig with one stdio server.
fn config_with_stdio_server(name: &str, command: &str, args: Vec<&str>) -> HubConfig {
    let mut servers = HashMap::new();
    servers.insert(
        name.to_string(),
        ServerConfig::Stdio {
            command: command.to_string(),
            args: args.into_iter().map(String::from).collect(),
            env: HashMap::new(),
            layer: mcp_hub::config::ServerLayer::Critical,
        },
    );
    HubConfig {
        mcp_servers: servers,
        ..HubConfig::default()
    }
}

/// Test: connect to a real MCP server via stdio, discover tools, call a tool.
///
/// Uses `npx @guanxiong/mcp-server-time` — skipped if npx unavailable.
#[tokio::test]
async fn test_stdio_transport_real_server_discover_tools() {
    if !command_exists("npx") {
        eprintln!("SKIP: npx not available on PATH");
        return;
    }

    let config = config_with_stdio_server(
        "time",
        "npx",
        vec!["-y", "@guanxiong/mcp-server-time@1.0.0"],
    );

    let proxy = ProxyServer::new(config);

    // Connect via stdio → should discover real tools
    proxy
        .connect_all()
        .await
        .expect("Should connect to time server via stdio");

    let tools = proxy.get_all_tools().await;
    assert!(
        !tools.is_empty(),
        "Should discover tools from the time server, got {} tools",
        tools.len()
    );

    // Verify at least one tool looks like a time tool (not a built-in)
    let time_tool = tools
        .iter()
        .find(|t| t.server_name == "time" && (t.original_name.contains("time") || t.original_name.contains("clock")))
        .expect("Should have a time-related tool from time server");
    assert_eq!(time_tool.server_name, "time");
    assert!(
        time_tool.name.starts_with("time___"),
        "Tool name should be prefixed with server name, got: {}",
        time_tool.name
    );

    eprintln!(
        "Discovered {} tools from time server: {:?}",
        tools.len(),
        tools.iter().map(|t| &t.original_name).collect::<Vec<_>>()
    );

    // Cleanup
    proxy.shutdown().await;
}

/// Test: connect to a real MCP server, call a tool, get a result.
#[tokio::test]
async fn test_stdio_transport_call_tool() {
    if !command_exists("npx") {
        eprintln!("SKIP: npx not available on PATH");
        return;
    }

    let config = config_with_stdio_server(
        "time",
        "npx",
        vec!["-y", "@guanxiong/mcp-server-time@1.0.0"],
    );

    let proxy = ProxyServer::new(config);
    proxy.connect_all().await.expect("Should connect");

    let tools = proxy.get_all_tools().await;
    assert!(!tools.is_empty(), "Should discover tools");

    // Find a time-server tool (skip built-in tools)
    let first_tool = tools.iter().find(|t| t.server_name == "time")
        .expect("Should have a tool from time server");
    eprintln!("Calling tool: {}", first_tool.name);

    let result = proxy
        .call_tool(&first_tool.name, None)
        .await
        .expect("Should call tool successfully");

    assert!(!result.is_empty(), "Tool call should return non-empty result");
    eprintln!("Tool result: {}", result);

    proxy.shutdown().await;
}

/// Test: connecting to a non-existent server should not crash the proxy.
/// The proxy should report 0 connected servers (graceful degradation).
#[tokio::test]
async fn test_nonexistent_server_graceful_degradation() {
    // Try connecting to an invalid command — should fail at transport level
    let config = config_with_stdio_server("bad", "this_command_does_not_exist_xyz", vec![]);

    let proxy = ProxyServer::new(config);

    // connect_all should return an error since no servers connected
    let result = proxy.connect_all().await;
    assert!(result.is_err(), "Should fail when no servers can connect");

    let servers = proxy.get_server_infos().await;
    let connected: Vec<_> = servers.iter().filter(|s| s.connected).collect();
    assert!(connected.is_empty(), "No servers should be connected, got: {:?}", connected);

    let tools = proxy.get_all_tools().await;
    // Built-in tools still present; no server tools should exist
    let server_tools: Vec<_> = tools.iter().filter(|t| t.server_name != "mcp-hub").collect();
    assert!(server_tools.is_empty(), "No server tools should be registered, got: {:?}", server_tools);
    assert!(!tools.is_empty(), "Built-in tools should still exist");
}

/// Test: HubConfig with no servers should still have built-in tools.
#[tokio::test]
async fn test_empty_config_with_builtins() {
    let config = HubConfig::default();
    assert!(config.mcp_servers.is_empty());

    let proxy = ProxyServer::new(config);
    proxy.connect_all().await.expect("Empty config should succeed");

    let tools = proxy.get_all_tools().await;
    // Built-in tools are always available
    assert!(tools.len() >= 2, "Should have at least 2 built-in tools, got {}", tools.len());
    let has_time = tools.iter().any(|t| t.name == "mcp-hub___get_current_time");
    assert!(has_time, "Should have built-in get_current_time tool");
}

/// Test: Call built-in time tool directly.
#[tokio::test]
async fn test_builtin_time_tool() {
    let config = HubConfig::default();
    let proxy = ProxyServer::new(config);
    proxy.connect_all().await.expect("Should connect (even with no servers)");

    let result = proxy.call_tool("mcp-hub___get_current_time", None).await
        .expect("Should call built-in time tool");

    assert!(result.contains("UTC"), "Should contain UTC, got: {}", result);
    eprintln!("Built-in time: {}", result);
}

/// Test: Call built-in ISO time tool.
#[tokio::test]
async fn test_builtin_time_iso() {
    let config = HubConfig::default();
    let proxy = ProxyServer::new(config);
    proxy.connect_all().await.expect("Should connect");

    let result = proxy.call_tool("mcp-hub___get_time_iso", None).await
        .expect("Should call ISO time tool");

    assert!(result.contains("T"), "Should be ISO format, got: {}", result);
    eprintln!("ISO time: {}", result);
}

/// Verify graceful shutdown cleans up without errors.
#[tokio::test]
async fn test_shutdown_cleans_up() {
    let config = HubConfig::default();
    let proxy = ProxyServer::new(config);
    proxy.connect_all().await.expect("Empty connect should succeed");
    proxy.shutdown().await; // Should not panic or hang
    assert!(proxy.is_shutting_down().await);
}
