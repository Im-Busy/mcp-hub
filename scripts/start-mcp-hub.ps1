# start-mcp-hub.ps1 — Auto-start mcp-hub if not already running
#
# Usage: .\scripts\start-mcp-hub.ps1
# Place in startup folder or run before opening OpenCode Desktop.
#
# Deploy to startup:
#   Copy-Item .\scripts\start-mcp-hub.ps1 "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup\"
#   Or create a Task Scheduler task triggered on logon.

param(
    [int]$Port = 9090,
    [string]$ConfigPath = "C:\Dev\projects\mcp-hub\mcp-hub.json",
    [string]$BinaryPath = "C:\Dev\projects\mcp-hub\target\release\mcp-hub.exe",
    [int]$StartupTimeout = 30
)

$ErrorActionPreference = "Continue"

# Check if already running
$existing = Get-Process mcp-hub -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "✅ mcp-hub already running (PID: $($existing.Id))" -ForegroundColor Green
    
    # Quick health check
    try {
        $health = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/health" -TimeoutSec 3
        Write-Host "   Status: $($health.status) | Servers: $($health.servers_connected)/$($health.servers_total) | Tools: $($health.tools_total)" -ForegroundColor Cyan
    } catch {
        Write-Host "   ⚠️ Health check failed — mcp-hub may be starting up" -ForegroundColor Yellow
    }
    exit 0
}

# Check if binary exists
if (-not (Test-Path $BinaryPath)) {
    Write-Host "❌ mcp-hub binary not found at: $BinaryPath" -ForegroundColor Red
    Write-Host "   Build first: cd C:\Dev\projects\mcp-hub; cargo build --release"
    exit 1
}

# Check if config exists
if (-not (Test-Path $ConfigPath)) {
    Write-Host "❌ Config not found at: $ConfigPath" -ForegroundColor Red
    exit 1
}

# Check port is free
$portCheck = netstat -ano | Select-String ":$Port "
if ($portCheck) {
    Write-Host "⚠️ Port $Port is already in use:" -ForegroundColor Yellow
    Write-Host $portCheck
    Write-Host "   mcp-hub may fail to bind. Change port with -Port <number>"
}

Write-Host "🚀 Starting mcp-hub on port $Port..." -ForegroundColor Cyan

# Start mcp-hub in background
$process = Start-Process -FilePath $BinaryPath `
    -ArgumentList "serve","--config",$ConfigPath,"--port",$Port `
    -WindowStyle Hidden `
    -PassThru

Write-Host "   PID: $($process.Id)" -ForegroundColor Gray

# Wait for health endpoint
$ready = $false
for ($i = 0; $i -lt $StartupTimeout; $i++) {
    Start-Sleep -Seconds 1
    try {
        $health = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/health" -TimeoutSec 2
        Write-Host "✅ mcp-hub ready!" -ForegroundColor Green
        Write-Host "   Status: $($health.status)" -ForegroundColor Cyan
        Write-Host "   Servers: $($health.servers_connected)/$($health.servers_total)" -ForegroundColor Cyan
        Write-Host "   Tools: $($health.tools_total)" -ForegroundColor Cyan
        Write-Host "   Endpoint: http://127.0.0.1:$Port/mcp" -ForegroundColor Cyan
        $ready = $true
        break
    } catch {
        Write-Host "   Waiting... ($($i+1)/$StartupTimeout seconds)" -ForegroundColor Gray
    }
}

if (-not $ready) {
    Write-Host "⚠️ mcp-hub started but health check timed out after ${StartupTimeout}s" -ForegroundColor Yellow
    Write-Host "   It may still be connecting to upstream servers (L1-L3)." -ForegroundColor Yellow
    Write-Host "   Check logs or run: mcp-hub status --config $ConfigPath" -ForegroundColor Yellow
}
