# mcp-hub

[![Rust](https://img.shields.io/badge/rust-edition%202021-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-active-brightgreen)](https://github.com/Im-Busy/mcp-hub)

![divider](https://readme-svg-wave-divider-generator.vercel.app/wave?type=sine&width=1200&height=100&amplitude=20&frequency=2&layers=2&color_top=f97316&color_bottom=7c2d12&opacity=1&flip=false&gradient=false&mirror=false&animate=false)

> Unified MCP management platform — proxy, package manager, inspector, and memory

## Why mcp-hub?

AI coding agents connect to MCP servers one-by-one — each with its own transport, config, and process. mcp-hub is **Layer 0** — a single endpoint that aggregates all MCP servers (Docker, npx/uvx, remote) behind one proxy. It combines patterns from 16 researched MCP repos into a polyglot architecture: Rust core + TypeScript Web UI + PowerShell orchestration.

![divider](https://readme-svg-wave-divider-generator.vercel.app/wave?type=sine&width=1200&height=100&amplitude=20&frequency=2&layers=2&color_top=f97316&color_bottom=7c2d12&opacity=1&flip=false&gradient=false&mirror=false&animate=false)

## Architecture

```mermaid
---
config:
  theme: neutral
  htmlLabels: false
---
flowchart TB
    subgraph CLIENTS [AI Clients — Cursor, Claude, VS Code]
        C1["JSON-RPC"]
    end

    CLIENTS --> PROXY

    subgraph PROXY [Layer 0 — mcp-hub Proxy]
        P1["`Aggregation · Prefixing\nTool routing · Parallel connect\nGraceful degradation`"]
    end

    PROXY --> MW
    subgraph MW [Middleware]
        M1["`Client: Logging · Security\nTool filter: allow/deny`"]
        M2["`Proxy: Description enricher\nTantivy BM25 tool search`"]
    end

    MW --> L1
    MW --> L2
    MW --> L3

    subgraph L1 [Layer 1 — Docker Services]
        D1["`graphiti · trendradr\nlangfuse · searxng`"]
    end

    subgraph L2 [Layer 2 — npx/uvx Tools]
        D2["`18 local servers\ntavily · exa · playwright\ngithub · context7`"]
    end

    subgraph L3 [Layer 3 — Remote]
        D3["`microsoft-learn\ntrendradr · graphiti`"]
    end

    style CLIENTS fill:#f97316,stroke:#c2410c,color:#fff
    style PROXY fill:#1a1a2e,stroke:#f97316,color:#e0e0e0
    style MW fill:#16213e,stroke:#f97316,color:#e0e0e0
    style L1 fill:#0f3460,stroke:#f97316,color:#e0e0e0
    style L2 fill:#0f3460,stroke:#f97316,color:#e0e0e0
    style L3 fill:#0f3460,stroke:#f97316,color:#e0e0e0
```

| Layer | Capabilities | Status |
|-------|-------------|:-----:|
| **Proxy Core** | Tool aggregation, server-name prefixing, tool routing, parallel connect, graceful degradation | ✅ |
| **Middleware** | Logging, security filtering, tool allow/deny, description enrichment, Tantivy BM25 search | ✅ |
| **Transports** | STDIO, HTTP Streamable, Named Pipe | ✅ |
| **Security** | AES-256-GCM encryption, lazy auth, rate limiting | 🟡 |
| **Package Mgmt** | MCPBX manifest, install/publish from registry | 🟡 |
| **Web UI** | Interactive inspector, server dashboard, config GUI | 🔜 |

![divider](https://readme-svg-wave-divider-generator.vercel.app/wave?type=sine&width=1200&height=100&amplitude=20&frequency=2&layers=2&color_top=f97316&color_bottom=7c2d12&opacity=1&flip=false&gradient=false&mirror=false&animate=false)

## Quick Start

```bash
# Auto-install
curl -fsSL https://raw.githubusercontent.com/Im-Busy/mcp-hub/main/install.sh | sh

# Run proxy
mcp-hub serve --config mcp-hub.example.json
npx mcp-hub serve --config mcp-hub.example.json

# Validate config
mcp-hub validate --config mcp-hub.example.json

# Check connected servers
mcp-hub status --config mcp-hub.example.json
```

| Platform | Install |
|----------|---------|
| **Cargo** | `cargo install mcp-hub` |
| **npx** | `npx mcp-hub serve` |
| **pip/uvx** | `uvx mcp-hub serve` |
| **Homebrew** | `brew install mcp-hub` |
| **Binary** | [GitHub Releases](https://github.com/Im-Busy/mcp-hub/releases) |

## Tech Stack

**Rust** · rmcp 1.7 · Axum 0.8 · Tokio · Tantivy 0.22 · Clap 4.5 · tracing · AES-256-GCM · zeroize

## License

Apache-2.0
