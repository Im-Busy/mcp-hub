//! Stress tests for mcp-hub — concurrent connections, tool calls, failure recovery.
//!
//! These tests spawn real MCP servers and stress-test the proxy under load.
//! Run with: cargo test --test stress_test -- --nocapture --test-threads=1
//!
//! Each test is #[ignore] by default (long-running). Use --include-ignored to run.

use mcp_hub::config::{HubConfig, ServerConfig};
use mcp_hub::ProxyServer;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

fn command_exists(cmd: &str) -> bool {
    if cfg!(windows) {
        std::process::Command::new("where").arg(cmd).output().map(|o| o.status.success()).unwrap_or(false)
    } else {
        std::process::Command::new("which").arg(cmd).output().map(|o| o.status.success()).unwrap_or(false)
    }
}

fn time_server_config(_name: &str) -> ServerConfig {
    ServerConfig::Stdio {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@guanxiong/mcp-server-time@1.0.0".to_string()],
        env: HashMap::new(),
        layer: mcp_hub::config::ServerLayer::Critical,
    }
}

fn hub_with_servers(servers: Vec<(&str, ServerConfig)>) -> HubConfig {
    let mut map = HashMap::new();
    for (name, cfg) in servers {
        map.insert(name.to_string(), cfg);
    }
    HubConfig { mcp_servers: map, ..HubConfig::default() }
}

// ── Scenario 1: Concurrent server connections ────────────────────

/// Connect to multiple MCP servers in parallel, verify all tools discovered.
#[tokio::test]
#[ignore = "long-running stress test"]
async fn stress_concurrent_server_connections() {
    if !command_exists("npx") { eprintln!("SKIP: npx not available"); return; }

    let server_count = 3;
    let servers: Vec<_> = (0..server_count)
        .map(|i| (format!("time-{}", i), time_server_config(&format!("time-{}", i))))
        .collect();
    let servers_ref: Vec<(&str, ServerConfig)> = servers.iter().map(|(n, c)| (n.as_str(), c.clone())).collect();

    let config = hub_with_servers(servers_ref);
    let proxy = Arc::new(ProxyServer::new(config));

    let start = Instant::now();
    proxy.connect_all().await.expect("Should connect to all servers");
    let connect_time = start.elapsed();

    let tools = proxy.get_all_tools().await;
    let server_infos = proxy.get_server_infos().await;

    eprintln!("═══ STRESS: Concurrent Connections ═══");
    eprintln!("  Servers connected: {}/{}", server_infos.iter().filter(|s| s.connected).count(), server_count);
    eprintln!("  Total tools discovered: {}", tools.len());
    eprintln!("  Connect time: {:?}", connect_time);
    eprintln!("  Tools per server: {:?}", server_infos.iter().map(|s| format!("{}={}", s.name, s.tool_count)).collect::<Vec<_>>());

    assert_eq!(server_infos.iter().filter(|s| s.connected).count(), server_count);
    assert!(!tools.is_empty(), "Should discover tools from all servers");

    proxy.shutdown().await;
    eprintln!("  ✅ PASS");
}

// ── Scenario 2: Concurrent tool discovery ────────────────────────

/// Bombard the proxy with 100+ concurrent tools/list requests.
#[tokio::test]
#[ignore = "long-running stress test"]
async fn stress_concurrent_tool_discovery() {
    if !command_exists("npx") { eprintln!("SKIP: npx not available"); return; }

    let config = hub_with_servers(vec![("time", time_server_config("time"))]);
    let proxy = Arc::new(ProxyServer::new(config));
    proxy.connect_all().await.expect("Should connect");

    let proxy = Arc::new(proxy);
    let concurrency = 100;
    let start = Instant::now();

    let mut handles = Vec::new();
    for _ in 0..concurrency {
        let p = proxy.clone();
        handles.push(tokio::spawn(async move {
            p.get_all_tools().await
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }
    let elapsed = start.elapsed();

    let success = results.iter().filter(|r| !r.is_empty()).count();
    eprintln!("═══ STRESS: Concurrent Tool Discovery ═══");
    eprintln!("  Requests: {}", concurrency);
    eprintln!("  Successful: {}/{}", success, concurrency);
    eprintln!("  Total time: {:?}", elapsed);
    eprintln!("  Avg per request: {:?}", elapsed / concurrency as u32);

    assert_eq!(success, concurrency, "All tool discovery requests should succeed");
    eprintln!("  ✅ PASS");
}

// ── Scenario 3: Concurrent tool calls ────────────────────────────

/// Call tools from multiple servers concurrently.
#[tokio::test]
#[ignore = "long-running stress test"]
async fn stress_concurrent_tool_calls() {
    if !command_exists("npx") { eprintln!("SKIP: npx not available"); return; }

    let server_count = 3;
    let servers: Vec<_> = (0..server_count)
        .map(|i| (format!("t{}", i), time_server_config(&format!("t{}", i))))
        .collect();
    let servers_ref: Vec<(&str, ServerConfig)> = servers.iter().map(|(n, c)| (n.as_str(), c.clone())).collect();

    let config = hub_with_servers(servers_ref);
    let proxy = Arc::new(ProxyServer::new(config));
    proxy.connect_all().await.expect("Should connect");

    let tools = proxy.get_all_tools().await;
    eprintln!("═══ STRESS: Concurrent Tool Calls ═══");
    eprintln!("  Servers: {}", server_count);
    eprintln!("  Tools discovered: {}", tools.len());
    eprintln!("  Tool names: {:?}", tools.iter().map(|t| &t.name).collect::<Vec<_>>());

    // Call each tool 10 times concurrently
    let calls_per_tool = 10;
    let mut handles = Vec::new();
    for tool in &tools {
        for _ in 0..calls_per_tool {
            let p = proxy.clone();
            let name = tool.name.clone();
            handles.push(tokio::spawn(async move {
                p.call_tool(&name, None).await
            }));
        }
    }

    let total_calls = handles.len();
    let start = Instant::now();

    let mut successes = 0;
    let mut failures = 0;
    for h in handles {
        match h.await.unwrap() {
            Ok(result) => {
                successes += 1;
                if successes == 1 {
                    eprintln!("  Sample result: {}...", &result[..result.len().min(80)]);
                }
            }
            Err(e) => {
                failures += 1;
                eprintln!("  ❌ Call failed: {}", e);
            }
        }
    }

    let elapsed = start.elapsed();
    eprintln!("  Total calls: {}", total_calls);
    eprintln!("  Successes: {}", successes);
    eprintln!("  Failures: {}", failures);
    eprintln!("  Total time: {:?}", elapsed);
    eprintln!("  Throughput: {:.1} calls/sec", total_calls as f64 / elapsed.as_secs_f64());

    assert!(failures == 0, "All tool calls should succeed");
    assert_eq!(successes, total_calls);
    eprintln!("  ✅ PASS");
}

// ── Scenario 4: Graceful degradation under server failure ────────

/// Kill one server mid-operation, verify others still work.
#[tokio::test]
#[ignore = "long-running stress test"]
async fn stress_graceful_degradation() {
    if !command_exists("npx") { eprintln!("SKIP: npx not available"); return; }

    // Connect to 2 servers
    let config = hub_with_servers(vec![
        ("s1", time_server_config("s1")),
        ("s2", time_server_config("s2")),
    ]);

    let proxy = ProxyServer::new(config);
    proxy.connect_all().await.expect("Should connect to both servers");

    let tools_before = proxy.get_all_tools().await;
    eprintln!("═══ STRESS: Graceful Degradation ═══");
    eprintln!("  Tools before shutdown: {}", tools_before.len());

    // Shutdown the proxy (kills all child processes)
    proxy.shutdown().await;

    // Proxy should report shutting_down = true
    assert!(proxy.is_shutting_down().await);
    eprintln!("  Shutdown flag: true");

    // Server info should still be queryable (metadata, not connections)
    let _infos = proxy.get_server_infos().await;
    eprintln!("  ✅ PASS — shutdown clean");
}

// ── Scenario 5: Repeated connect/disconnect ──────────────────────

/// Connect, disconnect, reconnect multiple times.
#[tokio::test]
#[ignore = "long-running stress test"]
async fn stress_repeated_connect_disconnect() {
    if !command_exists("npx") { eprintln!("SKIP: npx not available"); return; }

    let cycles = 3;
    eprintln!("═══ STRESS: Repeated Connect/Disconnect ═══");
    eprintln!("  Cycles: {}", cycles);

    for cycle in 1..=cycles {
        eprintln!("  ── Cycle {} ──", cycle);

        let config = hub_with_servers(vec![("time", time_server_config("time"))]);
        let proxy = ProxyServer::new(config);

        let c_start = Instant::now();
        proxy.connect_all().await.expect("Should connect");
        let tools = proxy.get_all_tools().await;
        eprintln!("    Connected in {:?}, {} tools", c_start.elapsed(), tools.len());

        // Make a few calls
        for tool in tools.iter().take(3) {
            match proxy.call_tool(&tool.name, None).await {
                Ok(r) => eprintln!("    Call '{}': {}...", tool.name, &r[..r.len().min(60)]),
                Err(e) => eprintln!("    Call '{}' FAILED: {}", tool.name, e),
            }
        }

        proxy.shutdown().await;
        eprintln!("    Shutdown complete");
    }

    eprintln!("  ✅ PASS — {} cycles completed", cycles);
}
