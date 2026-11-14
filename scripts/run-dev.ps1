# BandwidthX Quick Dev Runner
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Write-Host "Launching BandwidthX in Development Mode..." -ForegroundColor Cyan
npx tauri dev
