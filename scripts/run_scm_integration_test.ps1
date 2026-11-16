# ==============================================================================
# BandwidthX 2.3 — Windows SCM Lifecycle & WFP Integration Test Suite
# ==============================================================================
# This script MUST be executed in an elevated PowerShell prompt (Run as Administrator)
# to validate the real Windows Service Control Manager, fail-closed named pipe DACL,
# and genuine WFP kernel isolation.
# ==============================================================================

$ErrorActionPreference = "Stop"

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host " BandwidthX 2.3 Windows SCM & Real WFP Integration Test Suite" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan

# 1. Strict Administrator Privilege Assertion
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "CRITICAL: This test suite requires elevated administrative privileges to manipulate Windows SCM, WFP filters, and named pipe DACLs.`nPlease right-click PowerShell and select 'Run as Administrator', then rerun this script."
    exit 1
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$binDir = Join-Path $repoRoot "target\release"
$serviceExe = Join-Path $binDir "bandwidthx-service.exe"

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# 2. Build Release Workspace
Write-Host "`n[*] Building release workspace..." -ForegroundColor Yellow
Push-Location $repoRoot
try {
    & cargo build --release --workspace
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Release build failed with exit code $LASTEXITCODE"
        exit 1
    }
} finally {
    Pop-Location
}

if (-not (Test-Path $serviceExe)) {
    Write-Error "Failed to locate release binary at $serviceExe"
    exit 1
}

$serviceName = "BandwidthX"
$testSuccess = $false

try {
    # Clean slate: if service was left installed from an interrupted run, remove it
    $existing = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    if ($existing) {
        Write-Host "[*] Cleaning up pre-existing $serviceName service..." -ForegroundColor Yellow
        & $serviceExe --stop 2>$null
        Start-Sleep -Seconds 1
        & $serviceExe --uninstall 2>$null
        Start-Sleep -Seconds 1
    }

    Write-Host "`n[PHASE 1] Testing SCM Installation..." -ForegroundColor Green
    & $serviceExe --install
    if ($LASTEXITCODE -ne 0) {
        Write-Error "bandwidthx-service.exe --install failed with code $LASTEXITCODE"
        exit 1
    }

    $svc = Get-Service -Name $serviceName -ErrorAction Stop
    Write-Host "  -> SCM Registration Verified: $($svc.DisplayName) (Status: $($svc.Status))" -ForegroundColor Gray

    Write-Host "`n[PHASE 2] Testing SCM Service Startup..." -ForegroundColor Green
    & $serviceExe --start
    if ($LASTEXITCODE -ne 0) {
        Write-Error "bandwidthx-service.exe --start failed with code $LASTEXITCODE"
        exit 1
    }

    # Poll until SERVICE_RUNNING (timeout: 10s)
    $timeout = [DateTime]::UtcNow.AddSeconds(10)
    $running = $false
    while ([DateTime]::UtcNow -lt $timeout) {
        $svc.Refresh()
        if ($svc.Status -eq 'Running') {
            $running = $true
            break
        }
        Start-Sleep -Milliseconds 300
    }

    if (-not $running) {
        Write-Error "Service failed to reach Running state within 10 seconds (Current: $($svc.Status))"
        exit 1
    }
    Write-Host "  -> Service reached SERVICE_RUNNING successfully." -ForegroundColor Gray

    Write-Host "`n[PHASE 3] Verifying SCM Service Status Query..." -ForegroundColor Green
    & $serviceExe --status
    if ($LASTEXITCODE -ne 0) {
        Write-Error "bandwidthx-service.exe --status failed with code $LASTEXITCODE"
        exit 1
    }

    Write-Host "`n[PHASE 4] Executing Real SCM, Security & WFP Integration Suite (20 Tests)..." -ForegroundColor Green
    Push-Location $repoRoot
    try {
        & cargo test -p bandwidthx-service --test real_windows_scm_integration -- --test-threads=1 --nocapture
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Integration test assertions failed with exit code $LASTEXITCODE"
            exit 1
        }
    } finally {
        Pop-Location
    }

    Write-Host "`n[PHASE 5] Testing SCM Service Stop..." -ForegroundColor Green
    & $serviceExe --stop
    if ($LASTEXITCODE -ne 0) {
        Write-Error "bandwidthx-service.exe --stop failed with code $LASTEXITCODE"
        exit 1
    }

    # Poll until SERVICE_STOPPED (timeout: 10s)
    $timeout = [DateTime]::UtcNow.AddSeconds(10)
    $stopped = $false
    while ([DateTime]::UtcNow -lt $timeout) {
        $svc.Refresh()
        if ($svc.Status -eq 'Stopped') {
            $stopped = $true
            break
        }
        Start-Sleep -Milliseconds 300
    }

    if (-not $stopped) {
        Write-Error "Service failed to reach Stopped state within 10 seconds (Current: $($svc.Status))"
        exit 1
    }
    Write-Host "  -> Service reached SERVICE_STOPPED successfully." -ForegroundColor Gray

    Write-Host "`n[PHASE 6] Testing SCM Service Uninstallation..." -ForegroundColor Green
    & $serviceExe --uninstall
    if ($LASTEXITCODE -ne 0) {
        Write-Error "bandwidthx-service.exe --uninstall failed with code $LASTEXITCODE"
        exit 1
    }

    $uninstalled = $false
    try {
        Get-Service -Name $serviceName -ErrorAction Stop
    } catch {
        $uninstalled = $true
    }
    if (-not $uninstalled) {
        Write-Error "Service was not uninstalled from SCM"
        exit 1
    }
    Write-Host "  -> Service uninstalled from SCM successfully." -ForegroundColor Gray

    $testSuccess = $true
} finally {
    Write-Host "`n[*] Performing cleanup..." -ForegroundColor Yellow
    # Stop and uninstall if still active
    $lingering = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    if ($lingering) {
        if ($lingering.Status -ne 'Stopped') {
            & $serviceExe --stop 2>$null
            Start-Sleep -Milliseconds 500
        }
        & $serviceExe --uninstall 2>$null
    }

    # Clean up test child processes if any remain
    Get-Process -Name powershell -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -like "*Start-Sleep -Seconds 120*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

if ($testSuccess) {
    Write-Host "`n=================================================================" -ForegroundColor Cyan
    Write-Host " ALL 20 REAL WINDOWS SCM & WFP INTEGRATION TESTS PASSED (100%)" -ForegroundColor Cyan
    Write-Host "=================================================================" -ForegroundColor Cyan
    exit 0
} else {
    Write-Error "Real SCM integration test suite did not complete successfully."
    exit 1
}
