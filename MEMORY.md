# MEMORY.md — Persistent Handover State

> **Last Updated:** 2026-06-18

## Current Objective

Implement Phase 2 P1: BearerAuthClient decorator, smart caching (return cached + bg refresh), AES-256-GCM encrypted credentials. P0 (real rmcp transport connections + tool discovery) is complete.

## System State & Metrics

- **Project created:** 2026-06-18 (rescaffolded from project-initialize template)
- **Tech stack:** Rust (edition 2021), rmcp 1.7, Axum 0.8, Tantivy 0.22
- **Build tool:** cargo (rustup-managed toolchain)
- **Phase 1 status:** Complete — proxy aggregation, HTTP server, dual middleware, CLI, config system (7 modules, ~1,100 lines)
- **Phase 2 P0 status:** Complete — real rmcp transport connections (stdio + HTTP), tool discovery via list_tools()
- **Phase 2 P1+ status:** Stubbed — BearerAuthClient, smart caching, encrypted credentials, package mgmt, host integration
- **Tests:** 18 tests passing (13 unit + 5 integration)
- **Distribution plan:** Documented — crates.io + GitHub Releases (P0), npm/pip/Homebrew/Scoop/winget/uvx (P1), Docker (P2)

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
- [x] Phase 1: Clipboard memory system (stack + KV)
- [x] Phase 2 P0: Real rmcp STDIO transport (handler.serve via child process)
- [x] Phase 2 P0: Real rmcp HTTP transport (StreamableHttpClientTransport::from_uri)
- [x] Phase 2 P0: Real tool discovery (list_tools() replaces placeholder)
- [x] Phase 2 P0: ProxyServer.call_tool() executes real MCP calls
- [x] Phase 2 P0: Graceful shutdown (drop clients + kill child processes)
- [x] reqwest 0.12 → 0.13 upgrade (to match rmcp 1.7)
- [x] Project scaffolding completed (opencode.jsonc, kilo.json, .kilo/, .gitignore, README.md, MEMORY.md, DESIGN.md)
- [x] 8 reference repos cloned to useful_resources/useful-repos/
- [x] Comprehensive DESIGN.md (851 lines)
- [x] Complete distribution plan (10 channels)

## Discovered Issues & Blockers

- **BearerAuthClient disabled:** rmcp 1.7 StreamableHttpClient API changed (extra headers param in get_stream/post_message/delete_session). Needs adaptation.
- **No integration test with real MCP server:** Transports wired but not integration-tested against a real MCP server spawn.
- **CallToolRequestParams is non-exhaustive:** Can only construct via ::new(name) + field mutation.

## Next Session Agent Must

1. Read this MEMORY.md to understand current state
2. Review DESIGN.md for detailed module designs and implementation plan
3. Begin Phase 2 P1:
   - BearerAuthClient decorator (adapt to rmcp 1.7 API)
   - Smart caching middleware (cached + background refresh)
   - Integration tests with real MCP server spawn
4. Run `cargo test` before declaring any task complete

## Key Decisions

| Decision | Rationale | Date |
|----------|-----------|------|
| Rust single-binary, zero runtime deps | Cross-platform deployment, no Python/Node overhead | 2026-06-18 |
| Dual middleware (Client + Proxy) | Separates per-server from cross-server concerns — from MCProxy | 2026-06-18 |
| `server___tool` prefixing | Collision-free aggregation across servers — from MCProxy | 2026-06-18 |
| Config-driven middleware chains | Extensible without code changes — from MCProxy | 2026-06-18 |
| Initialized from project-initialize template | Standardized project structure | 2026-06-18 |
