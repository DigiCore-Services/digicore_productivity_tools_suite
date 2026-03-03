<#
.SYNOPSIS
    Reusable build script for DigiCore Text Expander (egui + Tauri binaries).

.DESCRIPTION
    Builds the DigiCore Text Expander project. Supports egui (native Rust UI),
    Tauri (web frontend + Rust backend), or both. Ensures npm dependencies
    are installed before Tauri build.

.PARAMETER Target
    Build target: All (default), Egui, Tauri

.PARAMETER Release
    Build in release mode (optimized).

.PARAMETER NoInstall
    Skip npm install for Tauri (use when deps are already installed).

.EXAMPLE
    .\build.ps1
    Builds both egui and Tauri binaries in debug mode.

.EXAMPLE
    .\build.ps1 -Target Tauri -Release
    Builds only the Tauri app in release mode.

.EXAMPLE
    .\build.ps1 -Target Egui
    Builds only the egui binary.
#>

[CmdletBinding()]
param(
    [ValidateSet("All", "Egui", "Tauri")]
    [string]$Target = "All",

    [switch]$Release,

    [switch]$NoInstall
)

$ErrorActionPreference = "Stop"

# Resolve digicore root (parent of scripts/)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DigicoreRoot = Split-Path -Parent $ScriptDir
$TauriAppDir = Join-Path $DigicoreRoot "tauri-app"

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host "    OK: $Message" -ForegroundColor Green
}

function Write-Fail {
    param([string]$Message)
    Write-Host "    FAIL: $Message" -ForegroundColor Red
}

function Build-Egui {
    Write-Step "Building egui binary (digicore-text-expander)"
    Push-Location $DigicoreRoot
    try {
        $cargoArgs = @("build", "-p", "digicore-text-expander")
        if ($Release) { $cargoArgs += "--release" }
        & cargo $cargoArgs
        if ($LASTEXITCODE -ne 0) {
            Write-Fail "cargo build failed"
            return $false
        }
        Write-Success "egui binary built"
        return $true
    }
    finally {
        Pop-Location
    }
}

function Build-Tauri {
    Write-Step "Building Tauri app"
    if (-not (Test-Path $TauriAppDir)) {
        Write-Fail "tauri-app directory not found: $TauriAppDir"
        return $false
    }

    # Ensure icon exists (required for Windows build)
    $iconPath = Join-Path $DigicoreRoot "tauri-app\src-tauri\icons\icon.ico"
    if (-not (Test-Path $iconPath)) {
        Write-Step "Creating icon.ico (required for Tauri)"
        $createIconScript = Join-Path $DigicoreRoot "tauri-app\scripts\create-icon.py"
        if (Test-Path $createIconScript) {
            & python $createIconScript
            if ($LASTEXITCODE -ne 0) {
                Write-Fail "Failed to create icon.ico"
                return $false
            }
        } else {
            Write-Fail "icon.ico not found and create-icon.py missing. Run: python tauri-app/scripts/create-icon.py"
            return $false
        }
    }

    if (-not $NoInstall) {
        Write-Step "Installing npm dependencies"
        Push-Location $TauriAppDir
        try {
            & npm install
            if ($LASTEXITCODE -ne 0) {
                Write-Fail "npm install failed"
                return $false
            }
            Write-Success "npm dependencies installed"
        }
        finally {
            Pop-Location
        }
    }

    Push-Location $TauriAppDir
    try {
        # npm run build runs "tauri build"; use -- to pass --release to tauri CLI
        if ($Release) {
            & npm run build -- --release
        } else {
            & npm run build
        }
        if ($LASTEXITCODE -ne 0) {
            Write-Fail "tauri build failed"
            return $false
        }
        Write-Success "Tauri app built"
        return $true
    }
    finally {
        Pop-Location
    }
}

# Main
Write-Host "DigiCore Text Expander - Build Script" -ForegroundColor White
Write-Host "Root: $DigicoreRoot" -ForegroundColor Gray
Write-Host "Target: $Target | Release: $Release" -ForegroundColor Gray

$failed = $false

switch ($Target) {
    "Egui"  { if (-not (Build-Egui)) { $failed = $true } }
    "Tauri" { if (-not (Build-Tauri)) { $failed = $true } }
    "All"   {
        if (-not (Build-Egui)) { $failed = $true }
        if (-not (Build-Tauri)) { $failed = $true }
    }
}

Write-Host ""
if ($failed) {
    Write-Host "Build completed with errors." -ForegroundColor Red
    exit 1
}
Write-Host "Build completed successfully." -ForegroundColor Green
exit 0
