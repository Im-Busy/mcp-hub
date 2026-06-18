# mcp-hub Dockerized MCP Servers
#
# Base image with Node.js + Python + uv, pre-installed MCP packages.
# Use with: docker run --rm -i mcp-hub-servers <entrypoint>
#
# Build: docker build -t mcp-hub-servers -f docker/Dockerfile .

FROM node:22-alpine

# Install Python + uv for uvx-based MCP servers
RUN apk add --no-cache python3 py3-pip && \
    pip3 install uv --break-system-packages

# ── Pre-install npm-based MCP servers ───────────────────────
# These are the ones that benefit most from pre-installation
RUN npm install -g \
    @modelcontextprotocol/server-memory@latest \
    @modelcontextprotocol/server-filesystem@latest \
    @modelcontextprotocol/server-sequential-thinking@latest \
    @modelcontextprotocol/server-github@latest \
    @upstash/context7-mcp@latest \
    task-master-ai@latest \
    @playwright/mcp@latest \
    tavily-mcp@latest \
    exa-mcp-server@latest

# ── Pre-cache uvx-based servers ─────────────────────────────
RUN uv tool install mcp-server-fetch && \
    uv tool install duckduckgo-mcp-server && \
    uv tool install markitdown-mcp

# ── Runtime ──────────────────────────────────────────────────
# No ENTRYPOINT — mcp-hub specifies the command per server
WORKDIR /workspace
