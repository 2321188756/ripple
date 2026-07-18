# Ripple Knowledge Service — Phase 0 local smoke
#
# Run this only with an isolated development PostgreSQL + pgvector instance.
# The service listens on loopback by default and never uses Ripple's desktop
# SQLite database or desktop API-key storage.

param(
    [Parameter(Mandatory = $true)]
    [string]$DatabaseUrl,

    [Parameter(Mandatory = $true)]
    [ValidateLength(32, 4096)]
    [string]$BootstrapToken,

    [string]$DataRoot = "D:\AI\Ripple\knowledge-service-data",

    [string]$ListenAddress = "127.0.0.1:8787"
)

$ErrorActionPreference = "Stop"

if (-not [System.IO.Path]::IsPathRooted($DataRoot)) {
    throw "DataRoot must be an absolute path."
}

$env:RIPPLE_KNOWLEDGE_DATABASE_URL = $DatabaseUrl
$env:RIPPLE_KNOWLEDGE_BOOTSTRAP_TOKEN = $BootstrapToken
$env:RIPPLE_KNOWLEDGE_DATA_ROOT = $DataRoot
$env:RIPPLE_KNOWLEDGE_LISTEN_ADDR = $ListenAddress
$env:RIPPLE_KNOWLEDGE_LOG = "ripple_knowledge_server=info,warn"

cargo run --manifest-path "$PSScriptRoot\..\..\src-tauri\Cargo.toml" -p ripple-knowledge-server
