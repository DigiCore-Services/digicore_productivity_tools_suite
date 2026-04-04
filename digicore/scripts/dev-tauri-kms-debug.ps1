<#
.SYNOPSIS
    Run Tauri dev with RUST_LOG tuned for KMS embedding / D6 migration troubleshooting.

.DESCRIPTION
    Sets RUST_LOG so the dedicated `kms_embed` log target (see embedding_service.rs) emits DEBUG,
    while keeping the rest of the app at INFO so you still see normal [KMS][D6] lines.

.PARAMETER RustLog
    Full RUST_LOG value. Default: info,kms_embed=debug

.EXAMPLE
    .\scripts\dev-tauri-kms-debug.ps1
    From digicore root: starts `npm run tauri -- dev` with KMS embed diagnostics.

.EXAMPLE
    .\scripts\dev-tauri-kms-debug.ps1 -RustLog "info,kms_embed=trace"
    Maximum verbosity for the kms_embed target only.

.NOTES
    KMS embedding failures are also appended to %APPDATA%\\DigiCore\\logs\\kms_embedding.log (see README).
    One-liner (PowerShell, from digicore\\tauri-app):
        $env:RUST_LOG='info,kms_embed=debug'; npm run tauri -- dev
#>

[CmdletBinding()]
param(
    [string]$RustLog = "info,kms_embed=debug"
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DigicoreRoot = Split-Path -Parent $ScriptDir
$TauriAppDir = Join-Path $DigicoreRoot "tauri-app"

if (-not (Test-Path -LiteralPath $TauriAppDir)) {
    Write-Error "tauri-app not found: $TauriAppDir"
}

$env:RUST_LOG = $RustLog
Write-Host "RUST_LOG=$($env:RUST_LOG)" -ForegroundColor Cyan
Write-Host "Starting Tauri dev from: $TauriAppDir" -ForegroundColor Cyan

Push-Location $TauriAppDir
try {
    npm run tauri -- dev
}
finally {
    Pop-Location
}
