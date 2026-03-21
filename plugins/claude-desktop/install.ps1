#Requires -Version 5.1
<#
.SYNOPSIS
    Context Keeper — Claude Desktop Plugin Installer (Windows)
.DESCRIPTION
    Detects or builds the context-keeper-mcp binary and registers it
    as an MCP server in Claude Desktop's configuration.
.PARAMETER Transport
    MCP transport mode: stdio, http, or both (default: stdio)
.PARAMETER HttpPort
    HTTP port when using http transport (default: 3000)
.PARAMETER BinaryPath
    Path to a pre-built context-keeper-mcp.exe binary
.PARAMETER Storage
    Storage backend: memory, rocksdb:<path> (default: memory)
.PARAMETER DbFilePath
    Path to context.sql persistence file (default: context.sql)
.PARAMETER ApiUrl
    OpenAI-compatible API base URL
.PARAMETER ApiKey
    API key for LLM services
.PARAMETER EmbeddingModel
    Embedding model name (default: text-embedding-3-small)
.PARAMETER EmbeddingDims
    Embedding dimensions (default: 1536)
.PARAMETER ExtractionModel
    Extraction model name (default: gpt-4o-mini)
.PARAMETER Uninstall
    Remove Context Keeper from Claude Desktop config
#>

param(
    [ValidateSet("stdio", "http", "both")]
    [string]$Transport = "stdio",

    [int]$HttpPort = 3000,

    [string]$BinaryPath = "",

    [string]$Storage = "memory",

    [string]$DbFilePath = "context.sql",

    [string]$ApiUrl = "",

    [string]$ApiKey = "",

    [string]$EmbeddingModel = "text-embedding-3-small",

    [int]$EmbeddingDims = 1536,

    [string]$ExtractionModel = "gpt-4o-mini",

    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Info  { param([string]$msg) Write-Host "[info]  $msg" -ForegroundColor Cyan }
function Write-Ok    { param([string]$msg) Write-Host "[ok]    $msg" -ForegroundColor Green }
function Write-Warn  { param([string]$msg) Write-Host "[warn]  $msg" -ForegroundColor Yellow }
function Write-Err   { param([string]$msg) Write-Host "[error] $msg" -ForegroundColor Red }

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = (Resolve-Path "$ScriptDir\..\..").Path

# ── Config path ──────────────────────────────────────────────────────────

$ConfigPath = Join-Path $env:APPDATA "Claude\claude_desktop_config.json"
$ConfigDir  = Split-Path -Parent $ConfigPath

# ── Uninstall ────────────────────────────────────────────────────────────

if ($Uninstall) {
    Write-Info "Removing Context Keeper from Claude Desktop config..."
    if (-not (Test-Path $ConfigPath)) {
        Write-Warn "Config file not found at $ConfigPath — nothing to do."
        exit 0
    }

    $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
    if ($cfg.mcpServers.PSObject.Properties["context-keeper"]) {
        $cfg.mcpServers.PSObject.Properties.Remove("context-keeper")
        $cfg | ConvertTo-Json -Depth 10 | Set-Content $ConfigPath -Encoding UTF8
        Write-Ok "Context Keeper removed from Claude Desktop config."
    } else {
        Write-Warn "Context Keeper not found in config — nothing to do."
    }
    Write-Info "Restart Claude Desktop to apply changes."
    exit 0
}

# ── Locate or build binary ───────────────────────────────────────────────

$Binary = ""

# 1. Explicit path
if ($BinaryPath -ne "") {
    if (Test-Path $BinaryPath) {
        $Binary = (Resolve-Path $BinaryPath).Path
        Write-Ok "Using specified binary: $Binary"
    } else {
        Write-Err "Specified binary not found: $BinaryPath"
        exit 1
    }
}

# 2. Check PATH
if ($Binary -eq "") {
    $found = Get-Command "context-keeper-mcp.exe" -ErrorAction SilentlyContinue
    if ($found) {
        $Binary = $found.Source
        Write-Ok "Found binary in PATH: $Binary"
    }
}

# 3. Check cargo target
if ($Binary -eq "") {
    $releaseBin = Join-Path $RepoRoot "target\release\context-keeper-mcp.exe"
    $debugBin   = Join-Path $RepoRoot "target\debug\context-keeper-mcp.exe"

    if (Test-Path $releaseBin) {
        $Binary = $releaseBin
        Write-Ok "Found release binary: $Binary"
    } elseif (Test-Path $debugBin) {
        Write-Warn "Found debug build — consider building with --release for better performance."
        $Binary = $debugBin
    }
}

# 4. Build from source
if ($Binary -eq "") {
    Write-Info "context-keeper-mcp binary not found."

    $cargo = Get-Command "cargo" -ErrorAction SilentlyContinue
    if (-not $cargo) {
        Write-Err "Rust toolchain not found. Please either:"
        Write-Err "  1. Install Rust: https://rustup.rs"
        Write-Err "  2. Build the binary and pass -BinaryPath <path>"
        exit 1
    }

    Write-Info "Building context-keeper-mcp from source (this may take a few minutes)..."
    Push-Location $RepoRoot
    try {
        cargo build --release -p context-keeper-mcp
    } finally {
        Pop-Location
    }

    $Binary = Join-Path $RepoRoot "target\release\context-keeper-mcp.exe"
    if (-not (Test-Path $Binary)) {
        Write-Err "Build completed but binary not found at $Binary"
        exit 1
    }
    Write-Ok "Built successfully: $Binary"
}

# ── Build config ─────────────────────────────────────────────────────────

$env_map = [ordered]@{
    STORAGE_BACKEND = $Storage
    DB_FILE_PATH    = $DbFilePath
}

if ($ApiUrl -ne "") {
    $env_map["OPENAI_API_URL"]   = $ApiUrl
    $env_map["OPENAI_API_KEY"]   = $ApiKey
    $env_map["EMBEDDING_MODEL"]  = $EmbeddingModel
    $env_map["EMBEDDING_DIMS"]   = [string]$EmbeddingDims
    $env_map["EXTRACTION_MODEL"] = $ExtractionModel
}

function Build-StdioConfig {
    return [ordered]@{
        command = $Binary
        args    = @("--transport", "stdio")
        env     = $env_map
    }
}

function Build-HttpConfig {
    return [ordered]@{
        url = "http://localhost:$HttpPort/mcp"
    }
}

# ── Write config ─────────────────────────────────────────────────────────

if (-not (Test-Path $ConfigDir)) {
    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
}

$cfg = @{}
if (Test-Path $ConfigPath) {
    $cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
}

# Ensure mcpServers exists
if (-not $cfg.PSObject.Properties["mcpServers"]) {
    $cfg | Add-Member -NotePropertyName "mcpServers" -NotePropertyValue (New-Object PSObject)
}

$serverConfig = switch ($Transport) {
    "stdio" { Build-StdioConfig }
    "http"  { Build-HttpConfig }
    "both"  { Build-StdioConfig }  # stdio as primary for Claude Desktop
}

# Convert to PSObject for proper JSON serialization
$serverObj = New-Object PSObject
foreach ($key in $serverConfig.Keys) {
    $serverObj | Add-Member -NotePropertyName $key -NotePropertyValue $serverConfig[$key]
}

# Add or replace the context-keeper entry
if ($cfg.mcpServers.PSObject.Properties["context-keeper"]) {
    $cfg.mcpServers.PSObject.Properties.Remove("context-keeper")
}
$cfg.mcpServers | Add-Member -NotePropertyName "context-keeper" -NotePropertyValue $serverObj

$cfg | ConvertTo-Json -Depth 10 | Set-Content $ConfigPath -Encoding UTF8

Write-Ok "Claude Desktop config updated at:"
Write-Info "  $ConfigPath"

# ── Summary ──────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host "  Context Keeper plugin installed for Claude Desktop" -ForegroundColor Green
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host ""
Write-Host "  Transport:  $Transport" -ForegroundColor Cyan
Write-Host "  Binary:     $Binary" -ForegroundColor Cyan
Write-Host "  Storage:    $Storage" -ForegroundColor Cyan

if ($ApiUrl -ne "") {
    Write-Host "  LLM:        $ExtractionModel via $ApiUrl" -ForegroundColor Cyan
} else {
    Write-Host "  LLM:        mock (set -ApiUrl and -ApiKey for real extraction)" -ForegroundColor Yellow
}

if ($Transport -eq "both" -or $Transport -eq "http") {
    Write-Host "  HTTP URL:   http://localhost:${HttpPort}/mcp" -ForegroundColor Cyan
}

Write-Host ""
Write-Host "  Restart Claude Desktop to activate the plugin." -ForegroundColor Yellow
Write-Host ""

if ($Transport -eq "both") {
    Write-Info "To also run the HTTP server (for other clients):"
    Write-Info "  $Binary --transport http --http-port $HttpPort"
    Write-Host ""
}
