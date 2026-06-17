# MEMORY.md — Persistent Handover State

> **Last Updated:** 2026-06-18

## Current Objective

Implement Phase 2 of mcp-hub: BearerAuthClient decorator, smart caching (return cached + bg refresh), real tool discovery via rmcp's list_tools(), AES-256-GCM encrypted credentials, package management, and host integration.

## System State & Metrics

- **Project created:** 2026-06-18 (rescaffolded from project-initialize template)
- **Tech stack:** Rust (edition 2021), rmcp 1.7, Axum 0.8, Tantivy 0.22
- **Build tool:** cargo (rustup-managed toolchain)
- **Phase 1 status:** Complete — proxy aggregation, HTTP server, dual middleware, CLI, config system (7 modules, ~1,100 lines)
- **Phase 2 status:** Stubbed — module declarations exist, placeholders only
- **Tests:** 2 inline unit test modules (proxy.rs, clipboard.rs). No integration tests yet.

## Completed Tasks

- [x] Project scaffold initialized from C:\Dev\project-initialize template
- [x] Phase 1: Proxy aggregation with `server___tool` prefixing (MCProxy pattern)
- [x] Phase 1: Axum HTTP JSON-RPC server with /mcp, /health, /servers, /tools
- [x] Phase 1: Config-driven hub (stdio/http server configs, middleware specs)
- [x] Phase 1: Dual middleware architecture (ClientMiddleware + ProxyMiddleware)
- [x] Phase 1: 3 client middleware types (logging, security, tool_filter)
- [x] Phase 1: 2 proxy middleware types (description_enricher, tool_search)
- [x] Phase 1: Transport layer (TransportMode enum, STDIO, HTTP, Pipe stubs)
- [x] Phase 1: CLI with 7 subcommands (serve, status, validate, inspect, install, publish, host)
- [x] Phase 1: 16 structured error variants
- [x] Phase 1: Clipboard memory system (stack + KV) — implemented but flagged Phase 2
- [x] Project scaffolding completed (opencode.jsonc, kilo.json, .kilo/, .gitignore, README.md, MEMORY.md)

## Discovered Issues & Blockers

- **Placeholder tool discovery:** ProxyServer::connect_all() uses register_placeholder_tools() — registers one dummy "discover" tool per server. Needs real rmcp list_tools() integration.
- **No real transport connections:** TransportHandle created but never actually connected. rmcp client setup needed.
- **No integration tests:** Only inline unit tests in proxy.rs and clipboard.rs. Full integration test suite needed.
- **Existing Cargo.lock may be stale:** After fresh clone, `cargo build` will regenerate. Not committed to git (in .gitignore for Rust libs, but should be committed for binaries).
- **No git history:** Project was never committed before session data loss.

## Next Session Agent Must

1. Read this MEMORY.md to understand current state
2. Review AGENTS.md for architecture, module map, and design principles
3. Begin work on Phase 2 priority items:
   - Real MCP transport connections via rmcp
   - Tool discovery (list_tools) replacing placeholder
   - BearerAuthClient decorator for transparent authentication
4. Write integration tests alongside each new feature
5. Run `cargo test` before declaring any task complete

## Key Decisions

| Decision | Rationale | Date |
|----------|-----------|------|
| Rust single-binary, zero runtime deps | Cross-platform deployment, no Python/Node overhead | 2026-06-18 |
| Dual middleware (Client + Proxy) | Separates per-server from cross-server concerns — from MCProxy | 2026-06-18 |
| `server___tool` prefixing | Collision-free aggregation across servers — from MCProxy | 2026-06-18 |
| Config-driven middleware chains | Extensible without code changes — from MCProxy | 2026-06-18 |
| Initialized from project-initialize template | Standardized project structure | 2026-06-18 |
