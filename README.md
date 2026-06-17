# mcp-hub

> Unified MCP management platform — proxy, package manager, inspector, and memory

## Quick Start

```bash
# Build
cargo build --release

# Run with config
cargo run -- serve --config mcp-hub.example.json

# Validate config
cargo run -- validate --config mcp-hub.example.json

# Check status
cargo run -- status --config mcp-hub.example.json

# Run tests
cargo test
```

## Project Structure

```
mcp-hub/
├── src/
│   ├── main.rs          # Entry point, CLI dispatch
│   ├── lib.rs           # Library root, module declarations
│   ├── cli.rs           # Clap-based CLI with 7 subcommands
│   ├── config.rs        # HubConfig, ServerConfig (stdio/http enums)
│   ├── proxy.rs         # ProxyServer: aggregation, prefixing, routing
│   ├── server.rs        # Axum HTTP JSON-RPC server
│   ├── error.rs         # Structured error types (16 variants)
│   ├── transports/      # STDIO, HTTP, Pipe transport layer
│   ├── middleware/       # Dual middleware (Client + Proxy)
│   ├── package/         # Package management (Phase 2)
│   ├── hosts/           # Host integration (Phase 2)
│   ├── security/        # Auth, encryption, rate limiting (Phase 2)
│   └── tools/           # Built-in tools (clipboard, search)
├── tests/               # Test suite
├── scripts/             # CLI scripts
├── mcp-hub.example.json # Example configuration
├── Cargo.toml           # Rust manifest + dependencies
├── opencode.jsonc       # OpenCode project config
├── kilo.json            # Kilo compatibility config
├── AGENTS.md            # Agent instructions + architecture docs
├── MEMORY.md            # Persistent handover state
└── .gitignore
```

## Architecture

mcp-hub aggregates multiple MCP servers behind a single endpoint, providing:

- **Proxy aggregation** — Connect to multiple upstream MCP servers, aggregate tools with `server___tool` prefixing
- **Dual middleware** — ClientMiddleware (per-server) + ProxyMiddleware (cross-server)
- **Config-driven** — Everything configurable via JSON: servers, middleware chains, security rules
- **Built-in tools** — Clipboard memory (stack + KV), Tantivy BM25 tool search
- **Graceful degradation** — Proxy starts even if some upstream servers fail

## Tech Stack

- **Language:** Rust (edition 2021)
- **MCP Protocol:** rmcp 1.7
- **HTTP Server:** Axum 0.8 + Tower 0.5
- **Full-text search:** Tantivy 0.22
- **Security:** AES-256-GCM, zeroize
- **CLI:** Clap 4.5
- **Observability:** tracing + tracing-subscriber

## Phase Roadmap

| Phase | Status | Key Deliverables |
|-------|--------|------------------|
| **Phase 1** | ✅ Complete | Proxy aggregation, HTTP server, dual middleware, CLI, config system |
| **Phase 2** | ⏳ Stubbed | BearerAuthClient, smart caching, encrypted credentials, package management, host integration |
| **Phase 3** | 🔜 Planned | Stderr→MCP notification pipeline, Web UI inspector, interactive CLI |
