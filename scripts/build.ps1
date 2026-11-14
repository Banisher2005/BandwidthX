# BandwidthX Build Automation Script
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "  BandwidthX — Complete Production Build" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# 1. Build React/TypeScript Frontend
Write-Host "`n[1/3] Building Frontend..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Error "Frontend build failed"
    exit 1
}

# 2. Build Tauri Desktop Application
Write-Host "`n[2/3] Building Tauri 2 App..." -ForegroundColor Yellow
cargo build --release -p bandwidthx-app
if ($LASTEXITCODE -ne 0) {
    Write-Error "Tauri App build failed"
    exit 1
}

# 3. Build Windows Service
Write-Host "`n[3/3] Building Privileged Windows Service..." -ForegroundColor Yellow
cargo build --release -p bandwidthx-service
if ($LASTEXITCODE -ne 0) {
    Write-Error "Windows Service build failed"
    exit 1
}

Write-Host "`n✔ Build Completed Successfully!" -ForegroundColor Green
Write-Host "Executables located at:"
Write-Host " - target/release/bandwidthx-app.exe"
Write-Host " - target/release/bandwidthx-service.exe"
