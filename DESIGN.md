# mcp-hub — Comprehensive Design & Implementation Plan

> **Document Version:** 2.0 | **Date:** 2026-06-18 | **Status:** Phase 1 ✅, Phase 2 P0 ✅, Phase 2 P1+ Active
> **Architecture:** Hybrid — 5 critical servers direct + 16 proxy-managed through mcp-hub
> **References:** 23+ researched repos mapped to every module

---

## 1. System Overview

### 1.1 What Is mcp-hub?

mcp-hub is a **Layer 0 unifying MCP proxy** — a single endpoint that AI clients (Cursor, Claude Desktop, VS Code) connect to. It aggregates ALL MCP servers behind one interface, regardless of whether they run in Docker containers, as npx/uvx child processes, or as remote HTTP services.

### 1.2 The Problem It Solves

Without mcp-hub, each AI client must:
- Manage dozens of individual MCP server configurations
- Spawn separate subprocesses for each stdio-based MCP server
- Handle connection failures, process lifetime, and tool collisions individually
- Expose hundreds of tools with no search or filtering mechanism
- Have no unified security, logging, or caching layer

With mcp-hub:
- **One endpoint** for all AI clients → `POST /mcp` (JSON-RPC)
- **One process** manages all upstream MCP servers
- **Automatic tool prefixing** prevents name collisions (`github___create_issue`, `filesystem___read_file`)
- **Full-text search** via Tantivy BM25 when tools exceed the exposure limit
- **Dual middleware** — per-server (ClientMiddleware) and cross-server (ProxyMiddleware)
- **Graceful degradation** — proxy starts even if some upstream servers fail

### 1.3 Core Design Principles

| # | Principle | What It Means |
|---|-----------|---------------|
| 1 | **Steal the best, leave the rest** | Every pattern comes from a proven implementation; no dependency on small repos |
| 2 | **Right language per module** | Rust for core proxy, TypeScript for Web UI, PowerShell for orchestration |
| 3 | **Single binary, polyglot project** | One `mcp-hub` binary deploys everywhere; Web UI served as static files |
| 4 | **Config-driven everything** | Middleware chains, server connections, tool visibility all configurable |
| 5 | **Graceful degradation** | Proxy starts with what's available, never blocks on a single failure |
| 6 | **Dual middleware** | Per-server concerns (Client) and cross-server concerns (Proxy) separated |
| 7 | **Docker-first orchestration** | Heavy services → Docker; lightweight services → npx/uvx |

---

## 2. System Architecture

### 2.1 Polyglot Project Layout

```
mcp-hub/
│
├── core/                          ← Rust — The proxy engine
│   ├── src/
│   │   ├── main.rs                Entry point, CLI dispatch (186 lines)
│   │   ├── lib.rs                 Module declarations, ASCII architecture diagram (68 lines)
│   │   ├── cli.rs                 Clap CLI — 7 subcommands (115 lines)
│   │   ├── config.rs              HubConfig, ServerConfig, MiddlewareSpec (252 lines)
│   │   ├── proxy.rs               ProxyServer — aggregation engine (271 lines)
│   │   ├── server.rs              Axum HTTP JSON-RPC server (246 lines)
│   │   ├── error.rs               HubError — 16 structured variants (72 lines)
│   │   ├── transports/            Transport abstraction layer
│   │   │   ├── mod.rs             TransportMode enum + TransportHandle (75 lines)
│   │   │   ├── stdio.rs           Child process transport (stub)
│   │   │   ├── http_client.rs     Streamable HTTP transport (stub)
│   │   │   └── pipe.rs            Named pipe transport (stub)
│   │   ├── middleware/            Dual middleware system
│   │   │   ├── mod.rs             MiddlewareManager + MiddlewareRegistry (279 lines)
│   │   │   ├── client.rs          ClientMiddleware trait
│   │   │   ├── proxy_trait.rs     ProxyMiddleware trait
│   │   │   ├── client_middleware.rs  Logging, Security, ToolFilter (shared file)
│   │   │   └── proxy_middleware/
│   │   │       ├── mod.rs
│   │   │       ├── description_enricher.rs
│   │   │       └── tool_search.rs  Tantivy BM25 search engine (567 lines)
│   │   ├── package/               Package management
│   │   │   ├── mod.rs             (stub)
│   │   │   └── manifest.rs        MCPBX manifest format (59 lines)
│   │   ├── hosts/                 Host integration
│   │   │   ├── mod.rs             (stub)
│   │   │   └── atomic_write.rs    (stub)
│   │   ├── security/              Security layer
│   │   │   ├── mod.rs             (8 lines)
│   │   │   ├── encryption.rs      AES-256-GCM (stub)
│   │   │   ├── auth.rs            Lazy authentication (9 lines)
│   │   │   └── rate_limit.rs      (stub)
│   │   └── tools/                 Built-in tools
│   │       ├── mod.rs             (7 lines)
│   │       └── clipboard.rs       Stack + KV memory system (106 lines)
│   ├── tests/
│   │   └── integration_test.rs    5 integration tests
│   ├── Cargo.toml                 27 well-established crate dependencies
│   └── Cargo.lock
│
├── web-ui/                        ← TypeScript/React — Phase 3
│   ├── src/                       Visual inspector, tool tester, config GUI
│   └── package.json               (from inspector patterns)
│
├── scripts/                       ← PowerShell — Orchestration
│   ├── start-mcp-services.ps1     Docker service lifecycle
│   ├── check-mcp-health.ps1       Health check + enable + pre-cache
│   └── profile-opencode.ps1       Shell integration
│
├── useful_resources/useful-repos/  ← 8 cloned reference repos (gitignored)
│   ├── sparfenyuk-mcp-proxy/      Python, 2.6k★ — HTTP↔stdio bridge patterns
│   ├── mcpproxy-go/               Go, 258★ — Safety middleware patterns
│   ├── tbxark-mcp-proxy/          Go, 698★ — HTTP aggregation patterns
│   ├── punkpeye-mcp-proxy/        TypeScript, 264★ — SSE transport patterns
│   ├── MCProxy/                   Rust, 19★ — Original middleware architecture
│   ├── pluggedin-mcp-proxy/       TypeScript, 131★ — Caching + clipboard patterns
│   ├── inspector/                 TypeScript, 10k★ — Web UI reference (official MCP)
│   └── tool-cli/                  Rust, 16★ — Package management + encryption patterns
│
├── opencode.jsonc
├── kilo.json
├── AGENTS.md                      Agent instructions + architecture docs
├── MEMORY.md                      Persistent handover state
├── README.md                      Project overview
├── DESIGN.md                      This document
└── .gitignore
```

### 2.2 Layered MCP Management (L0-L3)

```
                    ┌──────────────────────────────────┐
                    │  AI Clients                       │
                    │  (Cursor, Claude, VS Code, etc.)  │
                    └──────────────┬───────────────────┘
                                   │ JSON-RPC over HTTP
                                   │ POST /mcp
                                   ▼
┌─────────────────────────────────────────────────────────────────┐
│                    LAYER 0 — mcp-hub Proxy                        │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │ Tool Registry │  │  Middleware  │  │  HTTP API            │   │
│  │ (prefix,      │  │  Chain       │  │  /mcp (JSON-RPC)     │   │
│  │  route,       │  │  (logging,   │  │  /health             │   │
│  │  aggregate)   │  │   security,  │  │  /servers            │   │
│  │               │  │   search,    │  │  /tools              │   │
│  │               │  │   enrich)    │  │                      │   │
│  └──────┬────────┘  └──────┬───────┘  └──────────────────────┘   │
│         │                  │                                       │
│  ┌──────┴──────────────────┴──────────────────────────────┐      │
│  │              Transport Layer                             │      │
│  │  STDIO (child process) │ HTTP (reqwest) │ Pipe (stub)   │      │
│  └──────┬─────────────────┬────────────────┬───────────────┘      │
└─────────┼─────────────────┼────────────────┼──────────────────────┘
          │                 │                │
          ▼                 ▼                ▼
┌──────────────────┐ ┌──────────────┐ ┌──────────────┐
│ LAYER 1          │ │ LAYER 2      │ │ LAYER 3      │
│ Docker Services  │ │ npx/uvx Local│ │ Remote       │
│ (4 services)     │ │ (18 servers) │ │ (3 servers)  │
├──────────────────┤ ├──────────────┤ ├──────────────┤
│ graphiti:8000    │ │ tavily       │ │ microsoft    │
│ trendradar:3334  │ │ exa          │ │   -learn     │
│ langfuse:3000    │ │ context7     │ │ trendradar   │
│ searxng:8080     │ │ playwright   │ │   (Docker)   │
│                  │ │ chrome-devt  │ │ graphiti     │
│ Docker Compose   │ │ github       │ │   (Docker)   │
│ Lifecycle        │ │ filesystem   │ │              │
│                  │ │ fetch        │ │ Connectivity │
│                  │ │ duckduckgo   │ │ checks       │
│                  │ │ memory       │ │              │
│                  │ │ sequential   │ │              │
│                  │ │ lean-ctx     │ │              │
│                  │ │ task-master  │ │              │
│                  │ │ markitdown   │ │              │
│                  │ │ penpot       │ │              │
│                  │ │ serena       │ │              │
│                  │ │ google-maps  │ │              │
│                  │ │ composer     │ │              │
│                  │ │ time         │ │              │
│                  │ │ firecrawl    │ │              │
│                  │ │              │ │              │
│                  │ │ check-mcp-   │ │              │
│                  │ │ health.ps1   │ │              │
└──────────────────┘ └──────────────┘ └──────────────┘
```

### 2.3 Data Flow — Tool Call Lifecycle

```
1. AI Client sends JSON-RPC to POST /mcp
   { "method": "tools/call", "params": { "name": "github___create_issue", "arguments": {...} } }

2. server.rs → handle_mcp_request() parses method + tool name

3. proxy.rs → route_tool_call("github___create_issue")
   → tool_routing HashMap lookup → returns "github"

4. proxy.rs → extract_server_from_prefixed("github___create_issue")
   → ("github", "create_issue")

5. ClientMiddleware chain (per-server "github"):
   → LoggingMiddleware: logs the call
   → SecurityMiddleware: regex-check arguments for blocked patterns
   → ToolFilterMiddleware: check allow/deny rules for github server

6. Transport → HTTP request to github MCP server or spawn stdio process

7. Upstream response received → ProxyMiddleware chain:
   → ToolSearchMiddleware: updates Tantivy index if needed
   → DescriptionEnricherMiddleware: appends "(via mcp-hub)"

8. server.rs → returns JSON-RPC response to AI Client
```

### 2.4 Error Architecture

```
HubError (enum, 16 variants)
├── Config(String)              — Invalid config file
├── Connection { server, msg }  — Upstream server unreachable
├── Transport(String)           — Transport layer failure
├── ToolNotFound(String)        — Tool name not in registry
├── ServerNotFound(String)      — Server not configured
├── Middleware(String)           — Middleware chain failure
├── SecurityViolation { rule, server } — Blocked by security middleware
├── InvalidToolFormat(String)   — Bad "server___tool" format
├── Io(std::io::Error)          — Filesystem error (From impl)
├── Json(serde_json::Error)     — JSON parse error (From impl)
├── Http(reqwest::Error)        — HTTP client error (From impl)
├── Mcp(String)                 — MCP protocol error
├── Package(String)             — Package management error
├── Host(String)                — Host integration error
├── Encryption(String)          — Crypto error
├── RateLimit(String)           — Rate limit exceeded
└── Other(String)               — Catch-all
```

---

## 3. Module-by-Module Design

### 3.1 Config System (`config.rs` — 252 lines) ✅

**Purpose**: Single source of truth for all proxy configuration.

```rust
HubConfig {
    mcp_servers: HashMap<String, ServerConfig>,  // Upstream servers
    http_server: HttpServerConfig,               // Proxy endpoint settings
    middleware: MiddlewareConfig,                 // Middleware chains
    registry: RegistryConfig,                    // Package registry
    hosts: HostsConfig,                          // Host integration
    tools: ToolsConfig,                          // Built-in tools
}
```

**ServerConfig** is a tagged enum:
- `Stdio { command, args, env }` — Spawn as child process
- `Http { url, authorization_token }` — Connect over HTTP

**MiddlewareConfig** supports per-server overrides — if a server has specific middleware, it completely replaces the default chain.

### 3.2 Proxy Engine (`proxy.rs` — 271 lines) ✅

**Purpose**: Core aggregation engine. Connects to upstream servers, maintains tool registry with name prefixing, routes tool calls.

**Key data structures:**
```rust
ProxyServer {
    config: HubConfig,
    servers: RwLock<HashMap<String, ConnectedServerInfo>>,    // Upstream servers
    tool_registry: RwLock<HashMap<String, AggregatedTool>>,   // Prefixed tool → info
    tool_routing: RwLock<HashMap<String, String>>,            // Tool → server
    shutting_down: RwLock<bool>,
}

AggregatedTool {
    name: String,              // "github___create_issue"
    original_name: String,     // "create_issue"
    description: String,
    server_name: String,       // "github"
    input_schema: Option<Value>,
}
```

**Critical function**: `prefix_tool_name("github", "create_issue")` → `"github___create_issue"`. The first `___` is the server/tool delimiter.

**Current limitation**: `connect_all()` uses `register_placeholder_tools()` which registers dummy "discover" tools. Needs real `rmcp::list_tools()` integration.

### 3.3 HTTP Server (`server.rs` — 246 lines) ✅

**Endpoints:**

| Route | Method | Handler | Purpose |
|-------|--------|---------|---------|
| `/mcp` | POST | `handle_mcp_request` | MCP JSON-RPC: initialize, tools/list, tools/call |
| `/health` | GET | `handle_health` | Version, connected servers, total tools |
| `/servers` | GET | `handle_list_servers` | Per-server: name, connected, tool_count, last_refresh |
| `/tools` | GET | `handle_list_tools` | All aggregated tools with server mapping |

**MCP Protocol Handlers:**
- `initialize` → Returns protocol version, capabilities (tools.listChanged, logging), server info
- `notifications/initialized` → Acknowledges initialization
- `tools/list` → Returns all aggregated tools with inputSchema
- `tools/call` → Routes to upstream server via `proxy.route_tool_call()`

CORS is configurable via `HttpServerConfig.cors_enabled` and `cors_origins`.

### 3.4 CLI (`cli.rs` + `main.rs` — 301 lines) ✅

**7 subcommands:**

| Command | Arguments | Phase | Description |
|---------|-----------|:---:|-------------|
| `serve` | `-c/--config`, `--host`, `--port`, `--no-cors` | ✅ | Start proxy server |
| `status` | `-c/--config` | ✅ | List servers + tool counts |
| `validate` | `-c/--config` | ✅ | Validate config syntax |
| `inspect` | `<target> [args...]` | ⏳ | Interactive inspector (Phase 3) |
| `install` | `<package> [--registry]` | ⏳ | Install from registry (Phase 2) |
| `publish` | `<path> [--registry]` | ⏳ | Publish package (Phase 2) |
| `host` | `<host> [--tool/--remove/--list]` | ⏳ | Host integration (Phase 2) |

Default behavior (no subcommand): runs `serve` with the config file.

### 3.5 Transport Layer (`transports/` — 75 lines) ✅

```rust
enum TransportMode { Stdio, Http, Pipe }

struct TransportHandle {
    mode: TransportMode,
    name: String,
    connected: bool,
    capabilities: TransportCapabilities,
}
```

**Status**: Module structure complete, actual transport connections are stubs. Phase 2 must implement real `rmcp` client/server connections for each transport type.

**Missing transports** (not yet designed):
- SSE (Server-Sent Events) — from punkpeye/mcp-proxy patterns
- Streamable HTTP bridge — from sparfenyuk/mcp-proxy patterns

### 3.6 Middleware System (`middleware/` — 279 lines) ✅

**Dual architecture:**

```
ClientMiddleware (per-server, runs BEFORE upstream call)
├── LoggingMiddleware      — Logs every tool call (configurable level)
├── SecurityMiddleware     — Regex-based tool blocking
└── ToolFilterMiddleware   — Allow/deny per server via regex

ProxyMiddleware (cross-server, runs AFTER aggregation)
├── DescriptionEnricherMiddleware  — Appends suffix to descriptions
└── ToolSearchMiddleware            — Tantivy BM25 full-text search (567 lines)
```

**MiddlewareAction<T>** — the result of middleware execution:
- `Continue` — Pass through unchanged
- `Replace(T)` — Substitute with modified value
- `Block(String)` — Reject with error message

**MiddlewareRegistry** — string-keyed factory pattern. New middleware types registered with `register_client_factory("name", |config| → Arc<dyn ClientMiddleware>)`.

**Tool Search Middleware** (`tool_search.rs`, 567 lines): The most sophisticated module. Implements Tantivy BM25 full-text search with:
- In-memory index (rebuilds on tool list changes)
- Multi-field indexing (name, description, server, combined text)
- Configurable threshold (0.0-1.0 relevance score)
- Selective exposure: only `max_tools_limit` tools shown initially; remaining discoverable via `search_available_tools`
- Boolean query parsing, phrase matching, field-specific queries

### 3.7 Built-in Tools (`tools/clipboard.rs` — 106 lines) ✅

**Clipboard Memory System** (from pluggedin-mcp-proxy patterns):

```rust
Clipboard {
    entries: HashMap<String, String>,  // Named KV store
    stack: Vec<String>,                // Auto-indexed stack
    next_index: usize,
}
```

Operations: `set(name, value)`, `get(name)`, `delete(name)`, `push(value)`, `pop()`, `list_entries()`, `stack_size()`.

**Not yet wired** as an MCP tool — needs to be registered in the tool registry and exposed via `tools/list`.

### 3.8 Package Management (`package/` — 59 lines) ⏳

**MCPBX Manifest** (`manifest.rs`):
```rust
McpbxManifest {
    name: String,              // "library/bash"
    version: String,           // semver
    description: String,
    transport: TransportType,  // Stdio | Http | Sse
    entry: String,             // Command or URL
    env: HashMap<String, String>,
    oauth: Option<OAuthConfig>,
    system_config: SystemConfig,
}
```

Phase 2 must implement: `install`, `publish`, registry integration.

### 3.9 Security (`security/` — stubs) ⏳

| Module | Purpose | Reference |
|--------|---------|-----------|
| `encryption.rs` | AES-256-GCM encrypted credential storage with zeroize | tool-cli patterns |
| `auth.rs` | Lazy auth: public discovery, auth-required tool calls | pluggedin patterns |
| `rate_limit.rs` | Per-client rate limiting | Standard pattern |

### 3.10 Host Integration (`hosts/` — stubs) ⏳

Phase 2 must implement: automatic host config generation for Claude Desktop, Cursor, VS Code, and other AI clients. Uses `atomic_write.rs` for safe config file updates (write to temp file → atomic rename).

### 3.11 Web UI (Phase 3, TypeScript/React) ❌

Not yet started. Design references: `inspector` (10k★, official MCP project). Features:
- Interactive tool testing (send tool calls with arguments, see responses)
- Server management dashboard (connection status, tool counts, health)
- Config editor GUI (visual config.json editor with validation)
- stderr pipeline visualizer (real-time stderr output from stdio servers)
- Tool search interface (GUI frontend for Tantivy search)

---

## 4. Implementation Plan

### 4.1 Phase 1 — Core Proxy (COMPLETE) ✅

**Status**: 7 modules, ~1,100 lines of Rust, 18 tests passing.

| # | Task | File(s) | Lines | Tests |
|---|------|---------|------:|------:|
| 1.1 | Config system (HubConfig, ServerConfig, MiddlewareSpec) | `config.rs` | 252 | 1 |
| 1.2 | Proxy aggregation engine (tool registry, prefix, route) | `proxy.rs` | 271 | 3 |
| 1.3 | Axum HTTP JSON-RPC server (4 endpoints) | `server.rs` | 246 | 0 |
| 1.4 | CLI with 7 subcommands (Clap) | `cli.rs` + `main.rs` | 301 | 0 |
| 1.5 | Structured error types (16 variants) | `error.rs` | 72 | 0 |
| 1.6 | Transport abstraction (TransportMode enum) | `transports/mod.rs` | 75 | 0 |
| 1.7 | Dual middleware system (Manager + Registry + 5 types) | `middleware/` | 279 | 0 |
| 1.8 | Tantivy BM25 tool search middleware | `tool_search.rs` | 567 | 6 |
| 1.9 | Clipboard memory system (stack + KV) | `clipboard.rs` | 106 | 4 |
| 1.10 | MCPBX manifest format | `manifest.rs` | 59 | 0 |
| 1.11 | Integration tests | `tests/integration_test.rs` | — | 5 |
| 1.12 | Project scaffolding (opencode, kilo, gitignore, README, MEMORY) | Root | — | — |

**Verification**: `cargo test` → 18/18 passing, `cargo build --release` → zero errors.

### 4.2 Phase 2 — Transport, Security, Packages (DESIGNED, NOT BUILT) ⏳

**Priority order** (most blocking first):

#### 2.1 Real Transport Connections (P0) ✅ COMPLETE

**Current state**: Real rmcp transport connections working. STDIO via child process, HTTP via StreamableHttpClientTransport. Tool discovery via list_tools(). Integration tests with real MCP server.

**Implementation**:
```
2.1.1: Implement rmcp StdioTransport in src/transports/stdio.rs ✅
       - Spawn child process, pipe stdin/stdout
       - Connect via handler.serve((stdout, stdin)) 
       - Process lifecycle: spawn, health, restart on crash
       - Stderr inherited (not piped) to prevent buffer deadlock

2.1.2: Implement rmcp HTTP transport in src/transports/http_client.rs ✅
       - StreamableHttpClientTransport::from_uri(url)
       - BearerAuthClient deferred (rmcp 1.7 API changes need adaptation)

2.1.3: Wire transports into ProxyServer::connect_all() ✅
       - connect_single_server() spawns real MCP connections
       - Tool discovery via list_tools() replaces placeholder
       - Parallel connection, partial failure handling
       - Layered startup: L1 Critical (3 retries) → L2 Standard (1 retry) → L3 Optional (0 retries)

2.1.4: Integration tests for real transport connections ✅
       - test_raw_stdio_connection: spawn time server, verify rmcp handshake
       - test_stdio_transport_real_server_discover_tools: discover real tools
       - test_stdio_transport_call_tool: call a real tool, get result
       - test_nonexistent_server_graceful_degradation: bad command, proxy still functional
       - Stress test: concurrent tool calls (1,158 calls/sec)
```

**Files**: `stdio.rs`, `http_client.rs`, `proxy.rs`, `tests/integration_test.rs`, `tests/stress_test.rs`

#### 2.2 BearerAuthClient Decorator (P1)

**Current state**: Auth token in ServerConfig but not used during transport.

**Implementation**:
```
2.2.1: Create BearerAuthClient wrapper in middleware/client.rs
       - Wraps rmcp client with automatic Authorization header injection
       - Reads token from ServerConfig::Http.authorization_token
       - Supports token refresh callback

2.2.2: Wire into transport connection
       - Apply auth wrapper before connecting
       - Handle 401 responses with refresh
```

**Reference**: MCProxy's `bearer_auth.rs`

#### 2.3 Smart Caching Middleware (P1)

**Current state**: No caching. Every tool call hits upstream.

**Implementation**:
```
2.3.1: Create CacheMiddleware in middleware/client_middleware.rs
       - Cache key: (tool_name, arguments_hash)
       - TTL-based expiration (configurable per tool)
       - Return cached result immediately, refresh in background
       - Invalid on tool list changes

2.3.2: Wire into client middleware chain
       - Register as "cache" type in MiddlewareRegistry
       - Configurable per server or globally
```

**Reference**: pluggedin-mcp-proxy caching patterns

#### 2.4 AES-256-GCM Encryption (P1)

**Current state**: `security/encryption.rs` is a stub.

**Implementation**:
```
2.4.1: Implement EncryptedCredential struct
       - AES-256-GCM via aes-gcm crate
       - nonce + ciphertext + auth_tag envelope
       - Serialize to/from config file
       - Zeroize on drop (zeroize crate)

2.4.2: Implement key management
       - Auto-generate encryption key on first use
       - Store key in OS keychain or config directory
       - Key rotation support

2.4.3: Wire into config loading
       - Decrypt credentials on config load
       - Encrypt on config save
```

**Reference**: tool-cli encryption patterns

#### 2.5 Package Management (P2)

**Implementation**:
```
2.5.1: Implement package registry client (package/mod.rs)
       - Fetch package index from registry URL
       - Download MCPBX manifests
       - Install packages (download + extract + configure)

2.5.2: Wire install/publish CLI subcommands
       - cargo run -- install library/bash
       - cargo run -- publish ./my-server

2.5.3: Implement dependency resolution
       - Transitive MCP server dependencies
       - Version constraints (semver)
```

#### 2.6 Host Integration (P2)

**Implementation**:
```
2.6.1: Implement host config generators (hosts/mod.rs)
       - Claude Desktop: ~/AppData/Roaming/Claude/claude_desktop_config.json
       - Cursor: .cursor/mcp.json
       - VS Code: .vscode/mcp.json
       - Generic: OpenCode, Kilo, Continue formats

2.6.2: Implement atomic_write.rs
       - Write to temp file first
       - Atomic rename to target path
       - Backup existing config before overwrite

2.6.3: Wire host CLI subcommand
       - cargo run -- host claude-desktop --tool github___create_issue
       - cargo run -- host cursor --list
```

#### 2.7 Rate Limiting (P3)

**Implementation**: Per-client rate limiting via token bucket algorithm. Configurable limits per server and per client IP.

### 4.3 Phase 3 — Web UI, Inspector, Advanced Transports (PLANNED) 🔜

#### 3.1 Web UI (TypeScript/React)

**Implementation**:
```
3.1.1: Initialize TypeScript/React project in web-ui/
       - Vite + React 18 + TypeScript
       - Served as static files from Rust core (embedded via include_dir! or separate serve)

3.1.2: Build interactive tool tester
       - Dropdown: select tool from aggregated list
       - JSON editor: input arguments
       - Execute button → POST /mcp tools/call
       - Response viewer: formatted JSON + timing

3.1.3: Build server dashboard
       - Card per upstream server (name, status, tool count, last refresh)
       - Health indicators (green/yellow/red)
       - Start/stop individual server connections

3.1.4: Build config editor
       - Visual mcp-hub.json editor
       - Schema-aware validation
       - Add/remove/edit servers + middleware

3.1.5: Build stderr pipeline visualizer
       - Real-time stderr output from stdio servers
       - Color-coded: stderr (red), stdout (green)
       - Filter by server

3.1.6: Embed or serve Web UI
       - Option A: Embed in binary via rust-embed
       - Option B: Serve from separate process, proxy by core
```

**Reference**: inspector (10k★, official MCP project)

#### 3.2 SSE Transport

**Implementation**: Server-Sent Events transport support based on punkpeye/mcp-proxy patterns.

#### 3.3 Streamable HTTP Bridge

**Implementation**: HTTP↔stdio bridging based on sparfenyuk/mcp-proxy patterns.

#### 3.4 Stderr→MCP Notification Pipeline

**Implementation**: Capture stderr from child process servers and pipe to MCP notifications, based on inspector patterns.

---

## 5. Pattern Lineage — Complete Reference

### 5.1 From MCProxy (Rust, 19★)

| Pattern | mcp-hub Implementation | File | Lines |
|---------|----------------------|------|------:|
| Dual middleware (Client + Proxy) | MiddlewareManager + Registry | `middleware/mod.rs` | 279 |
| Server name prefixing ("___") | `prefix_tool_name()`, `extract_server_from_prefixed()` | `proxy.rs` | 271 |
| Factory/registry middleware | `MiddlewareRegistry` with string-keyed factories | `middleware/mod.rs` | — |
| Axum HTTP JSON-RPC server | Axum router with /mcp, /health, /servers, /tools | `server.rs` | 246 |
| Config-driven middleware chains | `MiddlewareConfig` with per-server overrides | `config.rs` | 252 |
| Tantivy BM25 tool search | In-memory index, multi-field, threshold, selective exposure | `tool_search.rs` | 567 |
| BearerAuthClient decorator | Not yet built (Phase 2) | — | — |

### 5.2 From pluggedin-mcp-proxy (TypeScript, 131★)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| Clipboard memory (stack + KV) | `tools/clipboard.rs` — 106 lines ✅ |
| Smart caching (return cached + bg refresh) | Not yet built (Phase 2) |
| Lazy authentication | `security/auth.rs` — stub |

### 5.3 From tool-cli (Rust, 16★)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| MCPBX manifest format | `package/manifest.rs` — 59 lines ✅ |
| AES-256-GCM encryption | `security/encryption.rs` — stub |
| Package management | `package/mod.rs` — stub |
| Host integration | `hosts/` — stub |

### 5.4 From sparfenyuk/mcp-proxy (Python, 2.6k★)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| Streamable HTTP↔stdio bridge | Phase 3 — design reference only |

### 5.5 From punkpeye/mcp-proxy (TypeScript, 264★)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| SSE transport | Phase 3 — design reference only |

### 5.6 From mcpproxy-go (Go, 258★)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| Safety-first middleware | Phase 2 — security middleware reference |

### 5.7 From inspector (TypeScript, 10k★, OFFICIAL)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| Web UI architecture | Phase 3 — primary reference |
| stderr pipeline | Phase 3 — design reference |

### 5.8 From mcp-proxy-tool (PowerShell, 14★)

| Pattern | mcp-hub Implementation |
|---------|----------------------|
| TransportMode enum | `transports/mod.rs` — 75 lines ✅ |

---

## 6. Cargo.toml Dependency Audit

### 6.1 All 27 Dependencies — SAFE ✅

| Category | Crates | Backing Org |
|----------|--------|-------------|
| Async runtime | `tokio 1`, `tokio-util 0.7`, `futures-util 0.3`, `bytes 1` | Tokio |
| MCP Protocol | `rmcp 1.7` (client, server, macros, transports, auth) | MCP standard |
| HTTP framework | `axum 0.8`, `tower 0.5`, `tower-http 0.6`, `hyper 1` | Tokio |
| HTTP client | `reqwest 0.12` (rustls-tls) | Seanmonstar |
| Serialization | `serde 1`, `serde_json 1`, `toml 0.8`, `toml_edit 0.22` | Serde, toml-rs |
| CLI | `clap 4.5` (derive, env) | clap-rs (15k+ stars) |
| Full-text search | `tantivy 0.22` | Quickwit (12k+ stars) |
| Security | `aes-gcm 0.10`, `zeroize 1`, `rand 0.8`, `sha2 0.10`, `base64 0.22`, `uuid 1` | RustCrypto, UUID |
| Error handling | `thiserror 2`, `anyhow 1` | dtolnay |
| Observability | `tracing 0.1`, `tracing-subscriber 0.3` (env-filter, json) | Tokio |
| Async trait | `async-trait 0.1` | dtolnay |
| Utilities | `regex 1`, `chrono 0.4`, `dirs 6`, `url 2` | Various (all well-established) |
| Unix | `libc 0.2` (conditional) | Rust libc |

**Zero solo-dev or low-starred dependencies.**

### 6.2 Dependency Philosophy

We **never** depend on the researched repos. We study their patterns, reimplement in Rust, and credit the source. The only dependencies are well-established crates backed by organizations (Tokio, RustCrypto, dtolnay) or with 10k+ GitHub stars.

---

## 7. Testing Strategy

### 7.1 Current State

| Type | Count | Coverage |
|------|------:|----------|
| Unit tests (inline) | 13 | proxy.rs, clipboard.rs, tool_search.rs |
| Integration tests | 5 | tests/integration_test.rs |
| **Total** | **18** | Core logic only |

### 7.2 Required Tests (Before Phase 2 Complete)

| Module | Test Count | Focus |
|--------|:---:|-------|
| `transports/stdio.rs` | 5+ | Process spawn, tool discovery, graceful shutdown |
| `transports/http_client.rs` | 5+ | HTTP connection, auth injection, retry |
| `middleware/client_middleware.rs` | 10+ | Logging, security rules, tool filter, caching |
| `security/encryption.rs` | 8+ | Encrypt/decrypt roundtrip, key rotation, zeroize |
| `security/auth.rs` | 5+ | Public/private endpoints, token validation |
| `security/rate_limit.rs` | 5+ | Token bucket, burst, config reload |
| `package/mod.rs` | 8+ | Install, publish, dependency resolution |
| `hosts/mod.rs` | 6+ | Config generation per host, atomic write |
| **Target total** | **70+** | Before Phase 2 sign-off |

### 7.3 Test Patterns

- **Unit tests**: `#[cfg(test)] mod tests { ... }` inline, one per source file
- **Integration tests**: `tests/*.rs`, exercise full proxy lifecycle
- **Transport tests**: Spawn real MCP servers, connect, discover, call
- **Middleware tests**: Each middleware type tested independently with mock tools
- **Config tests**: Roundtrip (parse → serialize → parse), invalid config rejection

---

## 8. Distribution & Deployment

> Follows `C:\Dev\.opencode\standards\distribution-standards.md` framework.

**Target Consumers**: Developers (run mcp-hub as a service), AI clients (connect via JSON-RPC), end users (download binary).  
**Deployment Model**: CLI tool + long-running service (single binary).  
**Platforms**: Windows (x86_64), macOS (arm64/x86_64), Linux (x86_64).

**Architecture**: One Rust binary → all channels. All channels are URL pointers, thin wrappers, or package manifests pointing at the same GitHub Release binary.

### 8.1 All Distribution Channels

| Priority | Channel | Install Command | Package Type | Effort |
|:---:|---------|-----------------|-------------|:---:|
| 🔥 P0 | **crates.io** | `cargo install mcp-hub` | Rust crate (binary) | Low |
| 🔥 P0 | **GitHub Releases** | Download `.exe` / binary from releases | Pre-built binaries (Win/Mac/Linux) | Low |
| ✅ P1 | **npm/npx** | `npx mcp-hub serve` | npm wrapper → download binary | Low |
| ✅ P1 | **bun/bunx** | `bunx mcp-hub serve` | Free — bun runs npm natively | Zero |
| ✅ P1 | **pip** | `pip install mcp-hub` | Python wrapper → download binary | Medium |
| ✅ P1 | **uvx** | `uvx mcp-hub serve` | Python wrapper → download binary | Medium |
| ✅ P1 | **Homebrew** | `brew install mcp-hub` | Ruby formula → download macOS binary | Low |
| ✅ P1 | **Scoop** | `scoop install mcp-hub` | JSON manifest → download Windows binary | Low |
| ✅ P1 | **winget** | `winget install mcp-hub` | YAML manifest → download Windows .exe | Low |
| ⬜ P2 | **Docker** | `docker run ghcr.io/imbusy/mcp-hub serve` | Container image | Medium |

### 8.2 Wrapper Architecture

All P1 channels are thin wrappers pointing at GitHub Releases:

```
GitHub Release (.exe / macOS binary / linux binary)
     │
     ├── npm package.json   → npx mcp-hub serve
     │                        (detect platform → download binary → execute)
     │                        Also gives bun/bunx for free
     │
     ├── pip setup.py       → pip install mcp-hub / uvx mcp-hub serve
     │                        (Python wrapper, same download-and-execute pattern)
     │
     ├── Homebrew formula   → brew install mcp-hub
     │   (.rb → url + sha256 → download macOS binary)
     │
     ├── Scoop manifest     → scoop install mcp-hub
     │   (.json → url + hash → download Windows .exe → add to PATH)
     │
     └── winget manifest    → winget install mcp-hub
         (.yaml → InstallerUrl + InstallerSha256 → download Windows .exe)
```

### 8.3 Channels Explicitly Skipped

| Channel | Reason |
|---------|--------|
| Chocolatey | Redundant — winget + Scoop already cover Windows |
| apt / dnf / pacman | Distro-maintained, cannot self-publish |
| Snap / Flatpak | Sandboxed formats are for GUI apps, not CLI tools |
| conda-forge | Python-only — pip + uvx already cover this |
| Maven / Gradle | Not a JVM project |

### 8.4 Release Configuration

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### 8.5 Config File Convention

Default config path: `mcp-hub.json` in current directory. Override with `-c/--config`.

Example configuration already exists: `mcp-hub.example.json` (90 lines, 2 configured servers).

---

## 9. Success Criteria

### 9.1 Phase 1 Complete ✅

- [x] Proxy starts and aggregates multiple upstream servers
- [x] Tool name prefixing prevents collisions
- [x] HTTP JSON-RPC endpoint responds to initialize, tools/list, tools/call
- [x] Health, servers, tools endpoints work
- [x] CLI with serve, status, validate subcommands functional
- [x] Middleware chain executes (logging, security, tool_filter, description_enricher, tool_search)
- [x] Tantivy BM25 search indexes and queries tools
- [x] Clipboard memory system works
- [x] 18 tests pass, cargo build --release succeeds
- [x] Project scaffolding complete (opencode, kilo, git, README, MEMORY, DESIGN)

### 9.2 Phase 2 Gate

- [x] Real MCP transport connections (stdio spawn + HTTP connect via rmcp) — Phase 2 P0 ✅
- [x] Tool discovery replaces placeholder with real list_tools() ✅
- [ ] BearerAuthClient decorator injects auth headers
- [ ] Smart caching returns cached results with background refresh
- [ ] AES-256-GCM encryption for credential storage
- [ ] Package install/publish works end-to-end
- [ ] Host integration generates configs for Claude, Cursor, VS Code
- [ ] 70+ tests passing
- [ ] Integration tests cover full proxy lifecycle with real MCP servers

### 9.3 Phase 3 Gate

- [ ] Web UI serves interactive tool tester, server dashboard, config editor
- [ ] SSE transport support
- [ ] Streamable HTTP↔stdio bridge
- [ ] stderr→MCP notification pipeline
- [ ] Full end-to-end test: client → mcp-hub → upstream server → response → client

---

## 10. File Size Registry

| File | Lines | Status |
|------|------:|:---:|
| `tool_search.rs` | 567 | ✅ Largest module (Tantivy search) |
| `cli.rs` + `main.rs` | 301 | ✅ |
| `middleware/mod.rs` | 279 | ✅ |
| `proxy.rs` | 271 | ✅ |
| `config.rs` | 252 | ✅ |
| `server.rs` | 246 | ✅ |
| `clipboard.rs` | 106 | ✅ |
| `transports/mod.rs` | 75 | ✅ |
| `error.rs` | 72 | ✅ |
| `lib.rs` | 68 | ✅ |
| `manifest.rs` | 59 | ✅ |
| All others | <50 | Stubs |

**Total Rust source**: ~2,100 lines (Phase 1). Estimated Phase 2: +2,000 lines. Phase 3 (Web UI): ~3,000 lines TypeScript.

---

*This document is the canonical design reference for mcp-hub. Update it when architecture decisions change.*
