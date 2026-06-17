# mcp-hub

> Unified MCP management platform — proxy, package manager, inspector, and memory

## Installation

| Platform | Command |
|----------|---------|
| **Cargo (any)** | `cargo install mcp-hub` |
| **npm/npx (any)** | `npx mcp-hub serve` |
| **pip (any)** | `pip install mcp-hub` |
| **uvx (any)** | `uvx mcp-hub serve` |
| **Homebrew (macOS)** | `brew install mcp-hub` |
| **Scoop (Windows)** | `scoop install mcp-hub` |
| **winget (Windows)** | `winget install mcp-hub` |
| **Pre-built binary** | Download from [GitHub Releases](https://github.com/Im-Busy/mcp-hub/releases) |

Or build from source:

```bash
git clone https://github.com/Im-Busy/mcp-hub.git
cd mcp-hub
cargo build --release
```

## Quick Start

```bash
# Run the proxy server
mcp-hub serve --config mcp-hub.example.json

# Or via npx
npx mcp-hub serve --config mcp-hub.example.json

# Check status
mcp-hub status --config mcp-hub.example.json

# Validate config
mcp-hub validate --config mcp-hub.example.json

# Run tests (from source)
cargo test
```

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
| **Phase 2** | 🟡 In Progress | Real transport connections ✅, BearerAuthClient, smart caching, encrypted credentials, package management, host integration |
| **Phase 3** | 🔜 Planned | Stderr→MCP notification pipeline, Web UI inspector, interactive CLI |
