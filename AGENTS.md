# Project Guidelines — mcp-hub

## Purpose

Unified MCP management platform — Layer 0 proxy that aggregates all MCP servers (Docker, npx/uvx, remote) behind a single endpoint. Combines patterns from 16 researched MCP repos into a polyglot architecture: Rust core + TypeScript Web UI + PowerShell orchestration.

## Tech Stack

| Layer | Language | Framework | Purpose |
|-------|----------|-----------|---------|
| **Core Proxy** | Rust (edition 2021) | rmcp 1.7, Axum 0.8, Tokio, Tantivy 0.22 | Aggregation, routing, middleware, search |
| **Web UI (Phase 3)** | TypeScript / React | React 18+ | Visual inspector, tool testing, config GUI |
| **Orchestration** | PowerShell | Native Windows | Docker service mgmt, host integration |

**Language rationale:** Rust is the correct fit for the performance-critical proxy core (zero-cost abstractions, async performance, single-binary deployment). TypeScript is the only sensible choice for a Web UI. PowerShell handles Docker orchestration naturally on Windows.

## Session Start Protocol

Every new AI session MUST start by reading these files in order:

1. **`C:\Dev\.opencode\standards\workspace-conventions.md`** — Global coding, bash, git, and tooling standards
2. **`MEMORY.md`** — Persistent handover state: current objective, completed tasks, discovered issues, next session priorities
3. **`README.md`** — Project overview, structure, phase roadmap

## Rust Environment

This project uses **Cargo** for Rust package and environment management:

- `cargo build` — Compile the project
- `cargo build --release` — Optimized build (LTO, strip, single codegen unit)
- `cargo test` — Run all tests
- `cargo check` — Fast compile check (no binary output)
- `cargo run -- <subcommand>` — Build and run with CLI arguments
- `cargo fmt` — Format code with rustfmt
- `cargo clippy` — Lint with Clippy

**Release profile:** opt-level=3, LTO=true, codegen-units=1, strip=true.

## Coding Conventions

Follow `C:\Dev\.opencode\standards\workspace-conventions.md` for all base conventions. Rust-specific additions:

- Public items must have doc comments (`///` or `//!`)
- Use `thiserror` for error types (not manual Display impls)
- Prefer `&str` over `&String` in function parameters
- Use `tracing` crate for logging (not `println!` or `eprintln!`)
- Keep modules under ~300 lines; split if larger
- One public type per file for substantial types. Small helpers can share.

## Prohibited Patterns

- No `unwrap()` in production code — use `?`, `.ok_or()`, or proper error handling
- No `unsafe` blocks without a comment explaining why safe alternatives don't exist
- No hardcoded secrets, paths, or environment-specific values — use config or env vars
- No `println!` debugging in committed code — use `tracing::debug!`
- No `.gitkeep` files — use README.md or .gitignore patterns

## Testing Standards

- Every new public function must have at least one test
- Test edge cases: empty inputs, boundary conditions, error paths
- Unit tests live in the same file under `#[cfg(test)] mod tests { ... }`
- Integration tests live in `tests/` directory
- Run tests after implementation changes: `cargo test`
- If tests fail due to your changes, fix them before reporting completion

## TDD Protocol

1. **Red first** — Write a failing test BEFORE writing implementation code
2. **Minimal implementation** — Only enough code to make the test pass. No extra functionality
3. **Refactor after green** — Only refactor when all tests pass
4. **No untested code** — Every new function/struct must have a corresponding test
5. **Test edge cases** — Empty inputs, boundary conditions, error paths, None values
6. **Run tests before declaring done** — `cargo test` must pass before marking complete

## Task Management

Use **Task Master MCP** for all multi-step task tracking. Load skill `task-master-integration` for the full usage guide. Never use `todowrite` for tasks with 3+ steps — Task Master provides dependency tracking, validation, and status management.

## Search, Navigation, and I/O Strategy

### Tool Selection Priority

```
Code in THIS project? → grep/glob (native)
External library docs? → Context7
External web info? → SearXNG → Tavily/Exa
Scraping URLs? → Firecrawl
GitHub issues/PRs? → GitHub MCP
Task management? → Task Master MCP
```

> **Full decision tree:** Load skill `mcp-search-strategy` for the complete decision matrix.

### SearXNG-First Protocol

1. **DEFAULT to SearXNG** for all web queries
2. **ESCALATE to Tavily/Exa** only when SearXNG returns < 3 relevant results
3. **BUDGET**: Max 5 Tavily + 5 Exa web calls per session

## API Grounding Protocol (Anti-Hallucination)

Before using ANY Rust crate API you haven't seen in the current codebase:

1. **Check Cargo.toml/Cargo.lock** — is the crate a direct or transitive dependency?
2. **Check docs.rs** — SearXNG with `"<crate> <function> docs.rs"`
3. **Reject invented APIs** — if no source confirms the API exists, do not use it
4. **On compilation failure** with "not found" → cross-reference crate docs before retrying

## File Creation Guidelines

- Find the canonical parent file for the topic before creating new files
- Add as a subsection — append, don't sprawl
- Only create new files when no parent exists, the content is a new category, or the parent exceeds ~800 lines

## Anti-Rationalization Guardrails

| Rationalization | Reality |
|----------------|---------|
| "I'll add validation later" | Without validation, you don't know if anything works |
| "One more tweak will fix it" | Adding without validation adds noise. Verify, then add |
| "This is too small for a plan" | Any change touching multiple modules needs at least a task list |
| "I can skip the edge cases" | Edge cases are where bugs live. Handle them |
| "I'll add tests after it works" | Untested code is production debt |

---

# Architecture

## Polyglot Project Layout

```
mcp-hub/
├── src/                              ← Rust core (proxy, middleware, transport, search)
│   ├── main.rs                       Entry point, CLI dispatch
│   ├── lib.rs                        Library root, module declarations
│   ├── cli.rs                        Clap CLI (7 subcommands)
│   ├── config.rs                     HubConfig, ServerConfig (stdio/http)
│   ├── proxy.rs                      ProxyServer aggregation engine
│   ├── server.rs                     Axum HTTP JSON-RPC server
│   ├── error.rs                      Structured errors (16 variants)
│   ├── transports/                   STDIO, HTTP, Pipe transport layer
│   ├── middleware/                    Dual middleware (Client + Proxy)
│   ├── package/                      Package management (Phase 2)
│   ├── hosts/                        Host integration (Phase 2)
│   ├── security/                     Auth, encryption, rate limiting (Phase 2)
│   └── tools/                        Built-in tools (clipboard, search)
├── web-ui/                           ← TypeScript/React (Phase 3)
│   └── (from inspector patterns, served as static files by core)
├── scripts/                          ← PowerShell orchestration
│   ├── start-mcp-services.ps1        Docker service mgmt
│   ├── check-mcp-health.ps1          Health check + enable + pre-cache
│   └── profile-opencode.ps1          Shell integration
├── useful_resources/useful-repos/    ← 8 cloned reference repos (gitignored)
│   ├── sparfenyuk-mcp-proxy/         (Python, 2.6k★)
│   ├── mcpproxy-go/                  (Go, 258★)
│   ├── tbxark-mcp-proxy/             (Go, 698★)
│   ├── punkpeye-mcp-proxy/           (TypeScript, 264★)
│   ├── MCProxy/                      (Rust, 19★)
│   ├── pluggedin-mcp-proxy/          (TypeScript, 131★)
│   ├── inspector/                    (TypeScript, 10k★, OFFICIAL)
│   └── tool-cli/                     (Rust, 16★)
├── tests/                            Integration tests
├── Cargo.toml                        Rust manifest
├── opencode.jsonc                    OpenCode project config
├── kilo.json                         Kilo compatibility config
├── MEMORY.md                         Persistent handover state
└── .gitignore
```

---

## Layered MCP Management Architecture

mcp-hub is **Layer 0** — the unifying proxy that all AI clients connect to. It routes to three managed layers:

```
                    ┌──────────────────────────┐
                    │   AI Clients               │
                    │  (Cursor, Claude, VS Code) │
                    └──────────┬───────────────┘
                               │ JSON-RPC
                               ▼
┌─────────────────────────────────────────────────────────┐
│                LAYER 0 — mcp-hub (unifying proxy)        │
│  Aggregates all tools, prefixes names, routes calls      │
│  Middleware: logging, security, search, caching          │
└──────┬──────────────────┬──────────────────┬────────────┘
       │                  │                  │
       ▼                  ▼                  ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ LAYER 1      │  │ LAYER 2      │  │ LAYER 3      │
│ Docker       │  │ npx/uvx      │  │ Remote       │
│ (4 services) │  │ (18 local)   │  │ (3 servers)  │
├──────────────┤  ├──────────────┤  ├──────────────┤
│ graphiti     │  │ tavily       │  │ microsoft    │
│ trendradar   │  │ exa          │  │   -learn     │
│ langfuse     │  │ context7     │  │ trendradar   │
│ searxng      │  │ playwright   │  │ graphiti     │
│              │  │ github       │  │              │
│ Managed by:  │  │ filesystem   │  │ Verified by: │
│ docker       │  │ ... (13 more)│  │ connectivity │
│ compose      │  │              │  │ checks       │
│              │  │ Managed by:  │  │              │
│              │  │ check-mcp-   │  │              │
│              │  │ health.ps1   │  │              │
└──────────────┘  └──────────────┘  └──────────────┘
```

**Key scripts** (in `C:\Dev\scripts\`):
- `start-mcp-services.ps1` — Start/stop Docker MCP backends
- `check-mcp-health.ps1 -EnableAll -PreCache` — Enable all MCPs + pre-cache packages + health check

---

## Complete Feature Inventory

### CLI Commands (`src/cli.rs`)

| Command | Arguments | Status | Description |
|---------|-----------|:---:|-------------|
| `serve` | `--config`, `--host`, `--port`, `--no-cors` | ✅ | Start proxy server, aggregate all upstream MCPs |
| `status` | `--config` | ✅ | List connected servers and their tool counts |
| `validate` | `--config` | ✅ | Validate configuration file syntax |
| `inspect` | `<target> [args...]` | ⏳ | Interactive MCP server inspector (Phase 3) |
| `install` | `<package> [--registry]` | ⏳ | Install MCP server from registry (Phase 2) |
| `publish` | `<path> [--registry]` | ⏳ | Publish MCP server package (Phase 2) |
| `host` | `<host> [--tool/--remove/--list]` | ⏳ | Add/remove tools from host configs (Phase 2) |

### Proxy Core (`src/proxy.rs`)

| Feature | Status | Description |
|---------|:---:|-------------|
| Tool aggregation | ✅ | Connect to multiple upstream servers, merge tool lists |
| Server name prefixing | ✅ | `server___tool` format for collision-free routing |
| Tool routing | ✅ | `route_tool_call()` dispatches to correct upstream server |
| Parallel connect | ✅ | Simultaneous connection to all configured servers |
| Graceful degradation | ✅ | Proxy starts even if some upstream servers fail |
| Tool discovery | ⏳ | Currently placeholder — needs real rmcp `list_tools()` |
| Smart caching | ⏳ | Return cached results, refresh in background (Phase 2) |

### Transport Layer (`src/transports/`)

| Transport | File | Status | Description |
|-----------|------|:---:|-------------|
| **STDIO** | `stdio.rs` | ✅ | Spawn MCP servers as child processes |
| **HTTP (Streamable)** | `http_client.rs` | ✅ | Connect to HTTP-based MCP servers via reqwest |
| **Named Pipe** | `pipe.rs` | ⏳ | Unix socket / Windows named pipe transport |
| **SSE** | (not built) | ❌ | Server-Sent Events transport (Phase 2/3) |
| **Streamable HTTP bridge** | (not built) | ❌ | HTTP↔stdio bridging (Phase 3) |

### Middleware System (`src/middleware/`)

| Middleware | Type | File | Status | Description |
|-----------|------|------|:---:|-------------|
| **Logging** | Client | `client_middleware.rs` | ✅ | Logs every tool call with configurable level |
| **Security** | Client | `client_middleware.rs` | ✅ | Regex-based tool blocking (system commands, sensitive files) |
| **Tool Filter** | Client | `client_middleware.rs` | ✅ | Allow/deny tool access per server via regex |
| **Description Enricher** | Proxy | `description_enricher.rs` | ✅ | Appends suffix (e.g., "(via mcp-hub)") to tool descriptions |
| **Tool Search** | Proxy | `tool_search.rs` | ✅ | Tantivy BM25 full-text search, selective tool exposure (567 lines) |
| **BearerAuthClient** | Client | (not built) | ⏳ | Transparent auth token injection (Phase 2) |
| **Smart Cache** | Client | (not built) | ⏳ | Cache-first with background refresh (Phase 2) |

### Built-in Tools (`src/tools/`)

| Tool | File | Status | Description |
|------|------|:---:|-------------|
| **Clipboard Memory** | `clipboard.rs` | ✅ | Stack (push/pop) + KV (set/get/delete) for agent persistence |
| **search_available_tools** | (in `tool_search.rs`) | ✅ | Virtual tool for discovering tools beyond the exposure limit |

### Security (`src/security/`)

| Feature | File | Status | Description |
|---------|------|:---:|-------------|
| **AES-256-GCM Encryption** | `encryption.rs` | ⏳ | Encrypted credential storage with zeroize on drop |
| **Lazy Authentication** | `auth.rs` | ⏳ | Public tool discovery, auth-required tool calls |
| **Rate Limiting** | `rate_limit.rs` | ⏳ | Per-client rate limiting for proxy endpoint |

### Package Management (`src/package/`)

| Feature | File | Status | Description |
|---------|------|:---:|-------------|
| **MCPBX Manifest** | `manifest.rs` | ✅ | Superset of MCPB format with OAuth + system config |
| **Install** | `mod.rs` | ⏳ | Install MCP servers from registry (Phase 2) |
| **Publish** | `mod.rs` | ⏳ | Publish packages to registry (Phase 2) |

### Host Integration (`src/hosts/`)

| Feature | File | Status | Description |
|---------|------|:---:|-------------|
| **Atomic Write** | `atomic_write.rs` | ⏳ | Safe config file writes (temp + rename) |
| **Multi-host config** | `mod.rs` | ⏳ | Claude, Cursor, VS Code, etc. (Phase 2) |

### HTTP Server (`src/server.rs`)

| Endpoint | Method | Status | Description |
|----------|--------|:---:|-------------|
| `/mcp` | POST | ✅ | Main MCP JSON-RPC endpoint (initialize, tools/list, tools/call) |
| `/health` | GET | ✅ | Health check (version, connected servers, total tools) |
| `/servers` | GET | ✅ | List connected upstream servers with status |
| `/tools` | GET | ✅ | List all aggregated tools with server mapping |

### Web UI (Phase 3, TypeScript/React)

| Feature | Status | Reference |
|---------|:---:|-----------|
| Interactive tool testing | ❌ | inspector (10k★, official MCP project) |
| Server management dashboard | ❌ | inspector + mcpproxy-go |
| Config editor GUI | ❌ | zarrx-dev/mcp-manage concept |
| stderr pipeline visualizer | ❌ | inspector |
| Tool search interface | ❌ | (already have backend, need frontend) |

---

## Pattern Lineage — 16 Researched Repos

### Phase 1 — Fully Implemented Patterns

| Pattern | Source | mcp-hub File | Lines |
|---------|--------|-------------|------:|
| Dual middleware (Client + Proxy) | MCProxy | `middleware/mod.rs` | 279 |
| Transport enum + dispatch | mcp-proxy-tool | `transports/mod.rs` | 75 |
| Server name prefixing ("___") | MCProxy | `proxy.rs` | 271 |
| Factory/registry middleware | MCProxy | `middleware/mod.rs` | 279 |
| Axum HTTP JSON-RPC server | MCProxy | `server.rs` | 246 |
| Config-driven middleware chains | MCProxy | `config.rs` | 252 |
| Tantivy BM25 tool search | MCProxy | `tool_search.rs` | 567 |
| Clipboard memory (stack + KV) | pluggedin | `clipboard.rs` | 106 |

### Phase 2 — Stubbed (Designed, Awaiting Implementation)

| Pattern | Source | Language | mcp-hub File |
|---------|--------|----------|-------------|
| BearerAuthClient decorator | MCProxy | Rust | `middleware/` (not built) |
| Smart caching (cached + bg refresh) | pluggedin | TypeScript | `middleware/` (not built) |
| AES-256-GCM encrypted credentials | tool-cli | Rust | `security/encryption.rs` |
| Lazy authentication | pluggedin | TypeScript | `security/auth.rs` |
| Rate limiting | — | Rust | `security/rate_limit.rs` |
| Package management (install/publish) | tool-cli | Rust | `package/mod.rs` |
| MCPBX manifest format | tool-cli | Rust | `package/manifest.rs` (done) |
| Host integration (Claude/Cursor) | tool-cli | Rust | `hosts/` |
| Transport bridging (--expose) | tool-cli | Rust | (not built) |

### Phase 3 — Planned (Not Started)

| Pattern | Source | Language | Notes |
|---------|--------|----------|-------|
| Stderr→MCP notification pipeline | inspector | TypeScript | Pipe stderr to notifications |
| Web UI for interactive inspection | inspector | TypeScript/React | Visual tool testing |
| SSE transport | punkpeye | TypeScript | Server-Sent Events |
| Streamable HTTP bridge | sparfenyuk | Python patterns | HTTP↔stdio bridging |

### Reference-Only (No Direct Dependency)

| Repo | Stars | Language | Value |
|------|------:|----------|-------|
| inspector | 10,116 | TypeScript | Web UI reference (official MCP project) |
| servers | 87,380 | TypeScript | Gold standard implementations (official MCP project) |
| awesome-mcp-servers | 4,168 | list | Curated discovery index |
| mcpproxy-go | 258 | Go | Safety middleware patterns |
| tbxark/mcp-proxy | 698 | Go | HTTP aggregation patterns |
| MediaPublishing/mcp-manager | 75 | JS | Web GUI concept only |
| nstebbins/mcp-manager | 25 | Python | CLI config manager concept (GPLv3) |
| house-mcp-manager | 8 | TypeScript | Minimal GUI concept |
| zarrx-dev/mcp-manage | 1 | TypeScript | Unified gateway concept |

---

## Dependency Philosophy

**Steal patterns, never depend on small repos.** Every pattern in mcp-hub comes from a proven implementation, reimplemented in the most suitable language for that module. Zero direct dependencies on any researched repo — only on well-established crates (Tokio, Axum, Tantivy, Clap, RustCrypto, dtolnay).

### Cargo.toml Dependency Posture (27 crates — all SAFE)

| Category | Crates | Backing |
|----------|--------|---------|
| Async runtime | tokio, tokio-util, futures-util, bytes | Tokio org |
| MCP Protocol | rmcp 1.7 | MCP standard Rust impl |
| HTTP | axum, tower, tower-http, hyper, reqwest | Tokio org + well-known |
| Serialization | serde, serde_json, toml, toml_edit | Industry standard |
| CLI | clap 4.5 | clap-rs (15k+ stars) |
| Search | tantivy 0.22 | 12k+ stars |
| Security | aes-gcm, zeroize, rand, sha2, base64, uuid | RustCrypto org |
| Error | thiserror, anyhow | dtolnay |
| Observability | tracing, tracing-subscriber | Tokio project |
| Utilities | chrono, dirs, url, regex, async-trait | All well-established |

---

## Distribution Channels

| Channel | Format | Target | Status |
|---------|--------|--------|:---:|
| **crates.io** | `cargo install mcp-hub` | Rust developers | 🔜 Planned |
| **GitHub Releases** | Pre-built binaries (Win/Mac/Linux) | End users | 🔜 Planned |
| **npm/npx** | Thin npm wrapper → download binary | Node.js ecosystem | Option |
| **Homebrew** | macOS formula | macOS users | Option |
| **Docker** | Container image | Docker-first users | Option |

**Recommended**: crates.io + GitHub Releases binaries. Cargo.toml already configured: `repository = "https://github.com/Im-Busy/mcp-hub"`.

---

## Quick Start

```bash
# Build
cd C:\Dev\projects\mcp-hub
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

---

## Design Principles

1. **Steal the best, leave the rest** — Every pattern comes from a proven implementation, studied across 4 languages
2. **Right language per module** — Rust for core proxy, TypeScript for Web UI, PowerShell for orchestration
3. **Single binary, polyglot project** — One `mcp-hub` binary deploys everywhere; Web UI served as static files
4. **Config-driven everything** — Middleware chains, server connections, tool visibility all configurable
5. **Graceful degradation** — If some servers fail, the proxy starts with what's available
6. **Dual middleware** — Per-server (ClientMiddleware) and cross-server (ProxyMiddleware) concerns separated
7. **Docker-first orchestration** — Heaviest services managed via Docker; lightweight services via npx/uvx
