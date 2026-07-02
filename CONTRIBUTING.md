# Contributing to mcp-hub

## Build

```bash
cargo build --release
cargo test
cargo clippy
```

## Architecture

mcp-hub is a Layer 0 MCP proxy that aggregates multiple MCP servers behind a single endpoint. See [README.md](README.md) for the full architecture diagram and feature inventory.

Source layout:

```
src/
├── main.rs          # Entry point, CLI dispatch
├── proxy.rs         # ProxyServer aggregation engine
├── server.rs        # Axum HTTP JSON-RPC server
├── config.rs        # HubConfig, ServerConfig
├── cli.rs           # Clap CLI (7 subcommands)
├── error.rs         # Structured error types
├── transports/      # STDIO, HTTP, Pipe transport
├── middleware/       # Dual middleware (Client + Proxy)
├── package/         # Package management
├── hosts/           # Host integration
├── security/        # Auth, encryption, rate limiting
└── tools/           # Built-in tools (clipboard, search)
```

## Tech Stack

Rust (edition 2021) · rmcp 1.7 · Axum 0.8 · Tokio · Tantivy 0.22 · Clap 4.5 · AES-256-GCM

## License

Apache-2.0