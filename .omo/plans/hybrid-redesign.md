# DESIGN.md — Hybrid Architecture (v2.0)

> See `C:\Dev\projects\mcp-hub\DESIGN.md` for the canonical design document.
> This `.omo/plans/` copy captures the hybrid architecture decision and comprehensive reference map.

## Hybrid Architecture — Final Decision

```
OpenCode MCP Config (6 entries):
├── github (direct, npx)          ← L1 Critical, always available
├── filesystem (direct, npx)      ← L1 Critical
├── context7 (direct, npx)        ← L1 Critical
├── tavily (direct, npx)          ← L1 Critical
├── exa (direct, npx)             ← L1 Critical
└── mcp-hub:9090/mcp (proxy)      ← 16 remaining servers
    ├── playwright(23), chrome-devtools(29), task-master(14)
    ├── memory(9), sequential-thinking(1), google-maps(2)
    ├── searxng(4), lean-ctx(10), microsoft-learn(3)
    ├── duckduckgo(2), composer-trade(26), markitdown(1)
    ├── penpot, serena, fetch(1)
    └── Built-in: get_current_time, get_time_iso, clipboard
```

**Redundancy**: Critical servers also exist in mcp-hub.json as fallback. If OpenCode's direct connection fails, mcp-hub still provides them.

**Why this design won**: Best balance of stability (critical tools survive mcp-hub crash), simplicity (only 6 entries in OpenCode config), and centralized management (mcp-hub handles the long tail).

## Task Status

- [x] Task 1: Add 5 critical servers back to OpenCode config
- [x] Task 2: Note redundancy (critical servers remain in both places)
- [x] Task 3: Update DESIGN.md v2.0 with hybrid architecture + comprehensive reference map
- [x] Task 4: Update AGENTS.md with hybrid decision
- [x] Task 5: Update MEMORY.md with current state

## Auto-Start

- [x] Task 6: Create start-mcp-hub.ps1 auto-start script (detect if running, start if not)
- [x] Task 7: Update start-all-mcp skill to reference mcp-hub auto-start

## Complete ✅

All 7 tasks done. Hybrid architecture deployed.
