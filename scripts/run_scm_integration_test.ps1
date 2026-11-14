# ==============================================================================
# BandwidthX 2.2 — Windows SCM Lifecycle & WFP Integration Test Suite
# ==============================================================================
# This script must be executed in an elevated PowerShell prompt (Run as Administrator)
# to validate the real Windows Service Control Manager and WFP kernel enforcement.
# ==============================================================================

$ErrorActionPreference = "Stop"

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host " BandwidthX 2.2 Windows SCM & Real WFP Integration Test Suite" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan

# 0. Check Administrator Privileges
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Warning "This test suite requires elevated administrative privileges to manipulate Windows SCM and WFP filters."
    Write-Warning "Please right-click PowerShell and select 'Run as Administrator', then rerun this script."
    exit 1
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$binDir = Join-Path $repoRoot "target\release"
$serviceExe = Join-Path $binDir "bandwidthx-service.exe"

if (-not (Test-Path $serviceExe)) {
    Write-Host "[*] Release binary not found. Building release workspace..." -ForegroundColor Yellow
    Push-Location $repoRoot
    cargo build --release --workspace
    Pop-Location
}

if (-not (Test-Path $serviceExe)) {
    Write-Error "Failed to locate $serviceExe after build."
    exit 1
}

Write-Host "`n[TEST 1] Installing Service in SCM..." -ForegroundColor Green
& $serviceExe --install
if ($LASTEXITCODE -ne 0) { Write-Error "Failed to install service"; exit 1 }

Write-Host "`n[TEST 2] Starting Service via SCM..." -ForegroundColor Green
& $serviceExe --start
if ($LASTEXITCODE -ne 0) { Write-Error "Failed to start service"; exit 1 }

Start-Sleep -Seconds 2

Write-Host "`n[TEST 3] Verifying SERVICE_RUNNING in SCM..." -ForegroundColor Green
& $serviceExe --status
if ($LASTEXITCODE -ne 0) { Write-Error "Failed to query service status"; exit 1 }

Write-Host "`n[TEST 4] Executing Real SCM Integration Suite..." -ForegroundColor Green
Push-Location $repoRoot
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo test -p bandwidthx-service --test real_windows_scm_integration -- --nocapture
if ($LASTEXITCODE -ne 0) { Write-Error "Integration test assertions failed"; exit 1 }
Pop-Location

Write-Host "`n[TEST 5] Stopping Service via SCM..." -ForegroundColor Green
& $serviceExe --stop
if ($LASTEXITCODE -ne 0) { Write-Error "Failed to stop service"; exit 1 }

Start-Sleep -Seconds 2

Write-Host "`n[TEST 6] Verifying SERVICE_STOPPED in SCM..." -ForegroundColor Green
& $serviceExe --status

Write-Host "`n[TEST 7] Uninstalling Service from SCM..." -ForegroundColor Green
& $serviceExe --uninstall
if ($LASTEXITCODE -ne 0) { Write-Error "Failed to uninstall service"; exit 1 }

Write-Host "`n=================================================================" -ForegroundColor Cyan
Write-Host " ALL REAL WINDOWS SCM & WFP INTEGRATION TESTS PASSED SUCCESSFULLY" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan
