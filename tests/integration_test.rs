//! Integration tests for mcp-hub.
//!
//! These tests exercise the public API through the library crate (`mcp_hub`).
//! Unit tests live alongside source in `src/` under `#[cfg(test)] mod tests`.

use mcp_hub::proxy::{extract_server_from_prefixed, prefix_tool_name};
use mcp_hub::HubConfig;
use mcp_hub::ProxyServer;

/// Verify the tool name prefix/extract roundtrip.
#[test]
fn test_tool_name_roundtrip() {
    let cases = vec![
        ("github", "create_issue"),
        ("filesystem", "read_file"),
        ("my-server", "my_tool_v2"),
        ("a", "b"),
    ];

    for (server, tool) in cases {
        let prefixed = prefix_tool_name(server, tool);
        let (extracted_server, extracted_tool) =
            extract_server_from_prefixed(&prefixed).unwrap();
        assert_eq!(extracted_server, server);
        assert_eq!(extracted_tool, tool);
    }
}

/// Verify that a tool name without "___" returns None.
#[test]
fn test_extract_invalid_format() {
    assert_eq!(extract_server_from_prefixed("no_separator"), None);
    assert_eq!(extract_server_from_prefixed(""), None);
}

/// Verify that "___" within the tool name is handled correctly
/// (only the first "___" is the server/tool boundary).
#[test]
fn test_extract_tool_with_underscores() {
    let result = extract_server_from_prefixed("srv___tool___with___underscores");
    assert_eq!(result, Some(("srv", "tool___with___underscores")));
}

/// Verify ProxyServer can be created and holds initial state.
#[test]
fn test_proxy_server_creation() {
    let config = HubConfig::default();
    let proxy = ProxyServer::new(config);

    // Initially no servers connected
    let rt = tokio::runtime::Runtime::new().unwrap();
    let servers = rt.block_on(proxy.get_server_infos());
    assert!(servers.is_empty());

    let tools = rt.block_on(proxy.get_all_tools());
    assert!(tools.is_empty());
}

/// Verify HubConfig default creates valid config.
#[test]
fn test_default_config() {
    let config = HubConfig::default();
    assert_eq!(config.http_server.host, "127.0.0.1");
    assert_eq!(config.http_server.port, 8081);
    assert!(config.http_server.cors_enabled);
    assert!(config.mcp_servers.is_empty());
    assert!(config.middleware.proxy.is_empty());
    assert!(config.middleware.client.default.is_empty());
}
