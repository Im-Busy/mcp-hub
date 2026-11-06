# Contributing to mcp-hub

## Build

```bash
cargo build --release
cargo test
cargo clippy
```

## Architecture

mcp-hub is an experimental Rust proxy for configured MCP upstreams. See
[README.md](README.md) for the implemented JSON HTTP interface and limitations.
`serve`, `status`, and `validate` have operational CLI branches; `inspect`,
`install`, `publish`, and `host` print placeholders. Module presence does not
establish that a feature is integrated into the running proxy.

Source layout:

```
src/
├── main.rs          # Entry point, CLI dispatch
├── proxy.rs         # ProxyServer aggregation engine
├── server.rs        # Axum HTTP JSON-RPC server
├── config.rs        # HubConfig, ServerConfig
├── cli.rs           # Clap CLI (7 subcommands)
├── error.rs         # Structured error types
├── transports/      # STDIO and HTTP; pipe connector is a placeholder
├── middleware/       # Dual middleware (Client + Proxy)
├── package/         # Partial package helpers; CLI install/publish are placeholders
├── hosts/           # Partial host helpers; CLI host branch is a placeholder
├── security/        # Security helpers; integration requires verification
└── tools/           # Time tools and separate clipboard helpers
```

## Tech Stack

Rust (edition 2021) · rmcp 1.7 · Axum 0.8 · Tokio · Tantivy 0.22 · Clap 4.5 · AES-256-GCM

## License

`Cargo.toml` declares MIT. The public snapshot has no top-level `LICENSE`
file. Confirm the intended license text with the owner; this documentation
does not supply or change a license grant. Preserve existing source attribution.
