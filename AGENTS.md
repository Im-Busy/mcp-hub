# Project Guidelines — mcp-hub

## Purpose

Unified MCP management platform — combines proxy aggregation (from MCProxy), package management (from tool-cli), inspection/debugging (from MCP Inspector), and memory tools (from pluggedin-mcp-proxy) into a single Rust binary.

## Tech Stack

Rust (edition 2021), rmcp 1.7, Axum 0.8, Tokio, Tantivy 0.22

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

Quick reference:
- `initialize_project` → set up task structure
- `add_task` → create tasks with dependencies
- `set_task_status` → mark in_progress / completed
- `get_next_task` → find the next unblocked task
- `parse_prd` → auto-generate tasks from a PRD

## Search, Navigation, and I/O Strategy

This project has access to multiple tool categories. Use the right tool for the task.

### Tool Selection Priority

```
Code in THIS project? → grep/glob (native)
External library docs? → Context7
External web info? → SearXNG → Tavily/Exa
Scraping URLs? → Firecrawl
GitHub issues/PRs? → GitHub MCP
Task management? → Task Master MCP
```

> **Full decision tree:** Load skill `mcp-search-strategy` for the complete decision matrix, escalation rules, and edge cases.

### SearXNG-First Protocol

1. **DEFAULT to SearXNG** for all factual, API, error, and version web queries
2. **ESCALATE to Tavily/Exa** only when SearXNG returns < 3 relevant results
3. **BUDGET**: Max 5 Tavily + 5 Exa web calls per session
4. **RESERVE Tavily** for complex multi-source research and time-sensitive news
5. **RESERVE Exa** for code examples and exact crate docs

## API Grounding Protocol (Anti-Hallucination)

Before using ANY Rust crate API you haven't seen in the current codebase:

1. **Check Cargo.toml/Cargo.lock** — is the crate a direct or transitive dependency?
2. **Check docs.rs** — SearXNG with `"<crate> <function> docs.rs"`
3. **Reject invented APIs** — if no source confirms the API exists, do not use it
4. **On compilation failure** with "not found" → cross-reference crate docs before retrying
5. **Cite sources** — every factual claim traceable to a URL or codebase reference

## File Creation Guidelines

Before creating a new file, ask: can this content be added to an existing file instead?

- Find the canonical parent file for the topic
- Add as a subsection — append, don't sprawl
- Only create new files when no parent exists, the content is a new category, or the parent exceeds ~800 lines
- Never create standalone files for status summaries, sub-experiments, or sequential phases

## Anti-Rationalization Guardrails

| Rationalization | Reality |
|----------------|---------|
| "I'll add validation later" | Without validation, you don't know if anything works |
| "One more tweak will fix it" | Adding without validation adds noise. Verify, then add |
| "This is too small for a plan" | Any change touching multiple modules needs at least a task list |
| "I can skip the edge cases" | Edge cases are where bugs live. Handle them |
| "I'll add tests after it works" | Untested code is production debt |
| "It works on my machine" | Cross-platform validation required |

---

# Architecture

This project combines patterns from 7 researched MCP management repos into a single Rust-based tool:

| Pattern | Source | Status |
|---------|--------|--------|
| Dual middleware (Client + Proxy) | MCProxy | ✅ Implemented |
| Transport enum + dispatch | mcp-proxy-tool | ✅ Implemented |
| Server name prefixing ("___") | MCProxy | ✅ Implemented |
| Factory/registry middleware | MCProxy | ✅ Implemented |
| Axum HTTP JSON-RPC server | MCProxy | ✅ Implemented |
| Config-driven middleware chains | MCProxy | ✅ Layered |
| BearerAuthClient decorator | MCProxy | ⏳ Phase 2 |
| Stderr→MCP notification pipeline | Inspector | ⏳ Phase 3 |
| Smart caching (return cached + bg refresh) | pluggedin | ⏳ Phase 2 |
| Tantivy BM25 tool search | MCProxy | ✅ Implemented |
| AES-256-GCM encrypted credentials | tool-cli | ⏳ Phase 2 |
| Host integration (Claude/Cursor/etc.) | tool-cli | ⏳ Phase 2 |
| Clipboard memory (stack + KV) | pluggedin | ✅ Core implemented |
| Transport bridging (--expose) | tool-cli | ⏳ Phase 2 |
| Package management (install/publish) | tool-cli | ⏳ Phase 2 |
| Web UI for interactive inspection | Inspector | ⏳ Phase 3 |

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

## Module Map

```
src/
├── main.rs         — Entry point, CLI dispatch
├── lib.rs          — Library root, module declarations
├── cli.rs          — Clap-based CLI with subcommands
├── config.rs       — HubConfig, ServerConfig (stdio/http enums)
├── proxy.rs        — ProxyServer: aggregation, prefixing, routing
├── server.rs       — Axum HTTP JSON-RPC server
├── error.rs        — Structured error types
├── transports/
│   ├── mod.rs      — TransportMode enum + TransportHandle
│   ├── stdio.rs    — Child process transport
│   ├── http_client.rs — Streamable HTTP transport
│   └── pipe.rs     — Named pipe transport (stub)
├── middleware/
│   ├── mod.rs      — MiddlewareManager + MiddlewareRegistry
│   ├── client.rs   — ClientMiddleware trait
│   ├── proxy_trait.rs — ProxyMiddleware trait
│   ├── client_middleware.rs — logging, security, tool_filter
│   └── proxy_middleware.rs  — description_enricher
├── package/
│   ├── mod.rs      — Package management (Phase 2)
│   └── manifest.rs — MCPBX manifest format
├── hosts/
│   ├── mod.rs      — Host integration (Phase 2)
│   └── atomic_write.rs
├── security/
│   ├── mod.rs      — Security utilities (Phase 2)
│   ├── encryption.rs — AES-256-GCM
│   ├── auth.rs     — Lazy authentication
│   └── rate_limit.rs — Rate limiting
└── tools/
    ├── mod.rs      — Built-in tools (Phase 2)
    └── clipboard.rs — Stack+KV clipboard memory
```

## Design Principles

1. **Steal the best, leave the rest** — Every pattern in this codebase comes from a proven implementation
2. **Rust-first, single-binary** — Zero runtime dependencies, cross-platform via Tokio
3. **Config-driven everything** — Middleware chains, server connections, tool visibility all configurable
4. **Graceful degradation** — If some servers fail, the proxy starts with what's available
5. **Dual middleware** — Per-server (ClientMiddleware) and cross-server (ProxyMiddleware) concerns separated
