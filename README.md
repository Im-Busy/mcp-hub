# mcp-hub

An experimental Rust proxy that connects to configured MCP servers, aggregates
their tools with `server___tool` names, and routes calls through a JSON HTTP
endpoint. This public source includes configuration validation, connection
status reporting, and built-in time tools.

The descriptions below reflect source inspection of the `public` branch on
November 7, 2026. They are not results of a build, runtime test, or MCP
conformance check.

## Current Capabilities

| Area | Implemented scope |
| --- | --- |
| Proxy | Configured upstream connections, tool aggregation, prefixed routing, connection retries, and health-monitor code |
| Upstream transports | Child-process stdio and rmcp Streamable HTTP client paths |
| HTTP listener | JSON routes for `/mcp`, `/health`, `/servers`, and `/tools` |
| CLI | `serve`, `status`, and `validate` have operational implementations |
| Built-in tools | Aggregated time tools; clipboard helpers exist separately in the library |
| Middleware | Library traits, factories, and implementations for logging, filters, enrichment, and search; their presence does not establish enforcement in the active HTTP route |

[`src/server.rs`](src/server.rs) dispatches a small set of methods, and
[`src/proxy.rs`](src/proxy.rs) performs upstream calls. The inbound listener
is a JSON POST interface. The upstream Streamable HTTP client does not make
the inbound interface a complete Streamable HTTP or MCP implementation.
Client compatibility, sessions, notifications, and protocol compliance need
separate verification.

## Build From A Source Checkout

Use Rust and Cargo with support for edition 2021, plus the build prerequisites
required by the declared dependencies. A first build needs registry access
when dependencies are not cached. No minimum compiler version or cross-platform
build certification is established by this documentation review.

```bash
git clone --depth 1 --single-branch --branch public https://github.com/Im-Busy/mcp-hub.git
cd mcp-hub
cargo build --release
```

Cargo declares the `mcp-hub` binary at [`src/main.rs`](src/main.rs) and the
`mcp_hub` library at [`src/lib.rs`](src/lib.rs). Registry publication and
Cargo/npm/Python/Homebrew packages or release binaries have not been verified.
The earlier `main/install.sh` URL does not target the public default branch;
the checked-in installer wrappers are outside this source-build procedure.

## Configuration And Commands

The loader accepts JSON using the structures in
[`src/config.rs`](src/config.rs). For a local listener without external
upstreams, use a project-local `hub.local.json` containing:

```json
{
  "mcp_servers": {},
  "http_server": {
    "host": "127.0.0.1",
    "port": 9090,
    "cors_enabled": false
  }
}
```

The default listener address in source is `127.0.0.1:9090`. This example
disables permissive CORS. Optional upstream entries use `type: "stdio"` with
`command`, `args`, and optional `env`, or `type: "http"` with `url` and an
optional `authorization_token`. Upstream commands, dependencies, permissions,
and credentials must match your environment.

[`mcp-hub.example.json`](mcp-hub.example.json) contains a broader sample with
environment-specific paths and credential placeholders. Review and replace
those values before using it; it is not a portable ready-to-run configuration.

```bash
# Parse the local JSON configuration; this does not connect to upstreams.
cargo run --release -- validate --config hub.local.json

# Open upstream connections and print status; this can spawn child processes.
cargo run --release -- status --config hub.local.json

# Start the HTTP listener and configured upstream connections.
cargo run --release -- serve --config hub.local.json
```

These commands describe the declared CLI and were not run in this review.
`serve` also accepts `--host`, `--port`, and `--no-cors`. `status` creates its
own connections; it does not query an already-running hub. Running the binary
without a subcommand starts `serve` behavior using the top-level `--config`
path, which defaults to `mcp-hub.json`.

Validation loads and deserializes JSON. It does not prove that executables,
URLs, credentials, middleware, or host integrations work.

## JSON HTTP Endpoints

| Method and path | Source-defined response |
| --- | --- |
| `POST /mcp` | Dispatches JSON-RPC methods `initialize`, `notifications/initialized`, `tools/list`, and `tools/call`; unknown methods receive a method-not-found error |
| `GET /health` | JSON status, version, total/connected server counts, and tool count; `status: "ok"` does not mean all upstreams connected |
| `GET /servers` | Server names, connection flags, tool counts, and last-refresh fields |
| `GET /tools` | Aggregated tool names, upstream names, original names, and descriptions |

For example, a tools-list POST body is:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list"}
```

`tools/call` uses a prefixed tool name in `params.name` and optional
`params.arguments`. The current HTTP handler wraps the proxy result as text;
do not assume transparent preservation of every upstream result shape.

## Roadmap And Unfinished Interfaces

| Interface | Current limitation |
| --- | --- |
| `inspect` | Prints its target/arguments and a Phase 3 placeholder |
| `install`, `publish` | Print inputs and Phase 2 placeholders; no CLI registry installation or publication |
| `host` | Prints options and a Phase 2 placeholder; no CLI host registration |
| Named pipes | `connect_pipe` returns an explicit not-implemented error; JSON upstream config only defines stdio and HTTP |
| Web UI | `web-ui/index.html` is a Phase 3 placeholder; the HTTP router does not serve an inspector or dashboard |
| Security, package, host, and clipboard modules | Partial library code does not imply a complete operational service or CLI workflow |

This checkout contains Rust source, an HTML placeholder, PowerShell helpers,
and Docker-related files. It does not provide the previously advertised
TypeScript Web UI or a verified universal host/platform setup.

## License And Attribution

[`Cargo.toml`](Cargo.toml) declares MIT. This public snapshot has no top-level
`LICENSE` file; the owner needs to supply the intended license text. The older
Apache-2.0 badge and statement disagreed with the manifest. This documentation
correction does not add or change a license grant.

Source comments credit patterns from MCProxy, tool-cli, MCP Inspector,
pluggedin-mcp-proxy, and mcp-proxy-tool. Preserve those attributions and check
dependency licenses separately. See [contributor notes](CONTRIBUTING.md) for
the source layout and development commands.
