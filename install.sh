#!/bin/sh
# mcp-hub universal installer — auto-detects OS/arch, picks best install method.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Im-Busy/mcp-hub/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/Im-Busy/mcp-hub/main/install.sh | sh -s -- --interactive
#
# Free hosting: served from GitHub raw content. No domain, no server, no cost.

set -e

REPO="Im-Busy/mcp-hub"
BOLD="\033[1m"
GREEN="\033[32m"
YELLOW="\033[33m"
CYAN="\033[36m"
RED="\033[31m"
RESET="\033[0m"

INTERACTIVE=false
[ "$1" = "--interactive" ] && INTERACTIVE=true

# ── Detect system ──────────────────────────────────────────────

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64)  ARCH="amd64" ;;
    aarch64|arm64)  ARCH="arm64" ;;
    *)              echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    linux)   BINARY="mcp-hub-linux-${ARCH}" ;;
    darwin)  BINARY="mcp-hub-macos-${ARCH}" ;;
    *)       echo "Unsupported OS: $OS"; exit 1 ;;
esac

echo "${BOLD}${GREEN}╔══════════════════════════════════════╗${RESET}"
echo "${BOLD}${GREEN}║   mcp-hub — vibe installer           ║${RESET}"
echo "${BOLD}${GREEN}╚══════════════════════════════════════╝${RESET}"
echo ""
echo "  ${CYAN}OS:${RESET}    $OS"
echo "  ${CYAN}Arch:${RESET}  $ARCH"
echo "  ${CYAN}Binary:${RESET} $BINARY"
echo ""

# ── Check available methods ────────────────────────────────────

has_cargo=false
has_npm=false
has_pip=false
has_brew=false

command -v cargo  >/dev/null 2>&1 && has_cargo=true
command -v npm    >/dev/null 2>&1 && has_npm=true
command -v pip    >/dev/null 2>&1 && has_pip=true
command -v brew   >/dev/null 2>&1 && has_brew=true

echo "  ${CYAN}Available package managers:${RESET}"
$has_cargo  && echo "    ✅ cargo"
$has_npm    && echo "    ✅ npm"
$has_pip    && echo "    ✅ pip"
$has_brew   && echo "    ✅ homebrew"
echo ""

# ── Interactive mode: let user choose ───────────────────────────

if $INTERACTIVE; then
    echo "  How would you like to install?"
    $has_cargo  && echo "    [1] cargo install mcp-hub"
    $has_npm    && echo "    [2] npm install -g mcp-hub"
    $has_pip    && echo "    [3] pip install mcp-hub"
    $has_brew   && echo "    [4] brew install mcp-hub"
    echo "    [d] Download binary directly (always works)"
    printf "  Choice [d]: "
    read CHOICE
    echo ""

    case "$CHOICE" in
        1) $has_cargo  && { cargo install mcp-hub; exit 0; } || echo "cargo not available" ;;
        2) $has_npm    && { npm install -g mcp-hub; exit 0; } || echo "npm not available" ;;
        3) $has_pip    && { pip install mcp-hub; exit 0; } || echo "pip not available" ;;
        4) $has_brew   && { brew install mcp-hub; exit 0; } || echo "brew not available" ;;
    esac
fi

# ── Auto: try each method, fall back to direct download ─────────

try_install() {
    echo "  Trying: $1"
    if eval "$2" 2>/dev/null; then
        echo "  ${GREEN}✅ Installed!${RESET}"
        exit 0
    fi
    echo "  ${YELLOW}⚠️  Not available or not yet published — trying next method...${RESET}"
}

try_install "cargo install mcp-hub" "cargo install mcp-hub"
try_install "npm install -g mcp-hub" "npm install -g mcp-hub 2>/dev/null && npx mcp-hub --version"
try_install "pip install mcp-hub" "pip install mcp-hub 2>/dev/null"
[ "$OS" = "darwin" ] && try_install "brew install mcp-hub" "brew install mcp-hub"

# ── Direct download from GitHub Releases ────────────────────────

echo ""
echo "  ${YELLOW}📦 Downloading binary directly from GitHub Releases...${RESET}"

# Get latest release tag
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
    | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
    echo "  ${RED}❌ Could not determine latest version.${RESET}"
    echo "  Build from source: git clone https://github.com/${REPO}.git && cd mcp-hub && cargo build --release"
    exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST}/${BINARY}.tar.gz"
echo "  Downloading: ${DOWNLOAD_URL}"

TMP_DIR=$(mktemp -d)
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/mcp-hub.tar.gz"
tar -xzf "$TMP_DIR/mcp-hub.tar.gz" -C "$TMP_DIR"

# Install to /usr/local/bin (or ~/.local/bin if no sudo)
if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

cp "$TMP_DIR/mcp-hub" "$INSTALL_DIR/mcp-hub"
chmod +x "$INSTALL_DIR/mcp-hub"
rm -rf "$TMP_DIR"

echo ""
echo "  ${GREEN}✅ mcp-hub ${LATEST} installed to ${INSTALL_DIR}/mcp-hub${RESET}"
echo ""
echo "  Try it: mcp-hub --version"
echo "  Start:  mcp-hub serve --config mcp-hub.json"

# Check if install dir is in PATH
case ":$PATH:" in
    *:"$INSTALL_DIR":*) ;;
    *)
        echo ""
        echo "  ${YELLOW}⚠️  ${INSTALL_DIR} is not in your PATH.${RESET}"
        echo "  Add this to your shell profile:"
        echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
