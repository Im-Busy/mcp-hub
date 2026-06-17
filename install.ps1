# mcp-hub universal installer — auto-detects system, picks best install method.
#
# Usage:
#   irm https://raw.githubusercontent.com/Im-Busy/mcp-hub/main/install.ps1 | iex
#   irm https://raw.githubusercontent.com/Im-Busy/mcp-hub/main/install.ps1 | iex -Args "--interactive"
#
# Free hosting: served from GitHub raw content. No domain, no server, no cost.

param(
    [switch]$Interactive
)

$ErrorActionPreference = "Stop"

$Repo = "Im-Busy/mcp-hub"

# ── Detect system ──────────────────────────────────────────────

$OS = if ($IsWindows) { "windows" } elseif ($IsMacOS) { "macos" } else { "linux" }
$Arch = if ([Environment]::Is64BitOperatingSystem) { "amd64" } else { "386" }

# PowerShell 5 fallback (doesn't have $IsWindows)
if (-not (Get-Variable IsWindows -Scope Global -ErrorAction SilentlyContinue)) {
    $OS = if ($env:OS -eq "Windows_NT") { "windows" } else { "linux" }
}

$Binary = if ($OS -eq "windows") { "mcp-hub-windows-${Arch}.exe" }
          else { "mcp-hub-${OS}-${Arch}" }

Write-Host "╔══════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║   mcp-hub — vibe installer           ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "  OS:    $OS" -ForegroundColor Cyan
Write-Host "  Arch:  $Arch" -ForegroundColor Cyan
Write-Host "  Binary: $Binary" -ForegroundColor Cyan
Write-Host ""

# ── Check available methods ────────────────────────────────────

$hasCargo   = $null -ne (Get-Command cargo   -ErrorAction SilentlyContinue)
$hasNpm     = $null -ne (Get-Command npm     -ErrorAction SilentlyContinue)
$hasPip     = $null -ne (Get-Command pip     -ErrorAction SilentlyContinue)
$hasScoop   = $null -ne (Get-Command scoop   -ErrorAction SilentlyContinue)
$hasWinget  = $null -ne (Get-Command winget  -ErrorAction SilentlyContinue)

Write-Host "  Available package managers:" -ForegroundColor Cyan
if ($hasCargo)  { Write-Host "    ✅ cargo" }
if ($hasNpm)    { Write-Host "    ✅ npm" }
if ($hasPip)    { Write-Host "    ✅ pip" }
if ($hasScoop)  { Write-Host "    ✅ scoop" }
if ($hasWinget) { Write-Host "    ✅ winget" }
Write-Host ""

# ── Interactive mode ────────────────────────────────────────────

if ($Interactive) {
    Write-Host "  How would you like to install?"
    $num = 1
    if ($hasCargo)  { Write-Host "    [$num] cargo install mcp-hub"; $num++ }
    if ($hasNpm)    { Write-Host "    [$num] npm install -g mcp-hub"; $num++ }
    if ($hasPip)    { Write-Host "    [$num] pip install mcp-hub"; $num++ }
    if ($hasScoop)  { Write-Host "    [$num] scoop install mcp-hub"; $num++ }
    if ($hasWinget) { Write-Host "    [$num] winget install mcp-hub"; $num++ }
    Write-Host "    [d] Download binary directly (always works)"
    $choice = Read-Host "  Choice [d]"

    if ($choice -match '^\d+$') {
        $idx = [int]$choice
        $methods = @()
        if ($hasCargo)  { $methods += @{ Name="cargo";  Cmd="cargo install mcp-hub" } }
        if ($hasNpm)    { $methods += @{ Name="npm";    Cmd="npm install -g mcp-hub" } }
        if ($hasPip)    { $methods += @{ Name="pip";    Cmd="pip install mcp-hub" } }
        if ($hasScoop)  { $methods += @{ Name="scoop";  Cmd="scoop install mcp-hub" } }
        if ($hasWinget) { $methods += @{ Name="winget"; Cmd="winget install mcp-hub" } }
        if ($idx -ge 1 -and $idx -le $methods.Count) {
            $m = $methods[$idx - 1]
            Write-Host "  Running: $($m.Cmd)" -ForegroundColor Yellow
            Invoke-Expression $m.Cmd
            exit 0
        }
    }
}

# ── Auto: try each method, fall back to direct download ─────────

function Try-Install {
    param($Name, $Cmd)
    Write-Host "  Trying: $Name" -ForegroundColor Yellow
    try {
        Invoke-Expression $Cmd *>$null
        Write-Host "  ✅ Installed!" -ForegroundColor Green
        exit 0
    } catch {
        Write-Host "  ⚠️  Not available — trying next method..." -ForegroundColor Yellow
    }
}

Try-Install "cargo install mcp-hub"  "cargo install mcp-hub"
Try-Install "npm install -g mcp-hub" "npm install -g mcp-hub"
Try-Install "pip install mcp-hub"    "pip install mcp-hub"
if ($OS -eq "windows") {
    Try-Install "scoop install mcp-hub"  "scoop install mcp-hub"
    Try-Install "winget install mcp-hub" "winget install mcp-hub"
}

# ── Direct download from GitHub Releases ────────────────────────

Write-Host ""
Write-Host "  📦 Downloading binary directly from GitHub Releases..." -ForegroundColor Yellow

$LatestJson = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -ErrorAction SilentlyContinue
if (-not $LatestJson) {
    Write-Host "  ❌ Could not determine latest version." -ForegroundColor Red
    Write-Host "  Build from source: git clone https://github.com/$Repo.git; cd mcp-hub; cargo build --release"
    exit 1
}

$Version = $LatestJson.tag_name
$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$Binary"
$InstallPath = if ($OS -eq "windows") { "$env:LOCALAPPDATA\mcp-hub\mcp-hub.exe" } else { "$env:HOME/.local/bin/mcp-hub" }

Write-Host "  Downloading: $DownloadUrl" -ForegroundColor Cyan
$null = New-Item -ItemType Directory -Force -Path (Split-Path $InstallPath)
Invoke-WebRequest -Uri $DownloadUrl -OutFile $InstallPath

if ($OS -ne "windows") { chmod +x $InstallPath }

Write-Host ""
Write-Host "  ✅ mcp-hub $Version installed to $InstallPath" -ForegroundColor Green
Write-Host ""
Write-Host "  Try it: mcp-hub --version"
Write-Host "  Start:  mcp-hub serve --config mcp-hub.json"

# Add to PATH if needed
if ($OS -eq "windows") {
    $binDir = Split-Path $InstallPath
    if ($env:PATH -notlike "*$binDir*") {
        Write-Host ""
        Write-Host "  ⚠️  $binDir is not in your PATH." -ForegroundColor Yellow
        Write-Host "  Run this to add it:"
        Write-Host "    [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$binDir', 'User')"
    }
}
