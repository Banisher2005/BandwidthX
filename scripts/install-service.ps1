# BandwidthX Windows Service Registration Script
param (
    [switch]$Install,
    [switch]$Uninstall,
    [switch]$Start,
    [switch]$Stop
)

$serviceName = "BandwidthXService"
$serviceDisplayName = "BandwidthX Privileged Network Service"
$binaryPath = Join-Path (Get-Location) "target\release\bandwidthx-service.exe"

if ($Install) {
    Write-Host "Installing BandwidthX Service..." -ForegroundColor Cyan
    if (!(Test-Path $binaryPath)) {
        Write-Error "Service binary not found at $binaryPath. Run scripts/build.ps1 first."
        exit 1
    }
    sc.exe create $serviceName binPath= "`"$binaryPath`"" start= auto DisplayName= "`"$serviceDisplayName`""
    sc.exe description $serviceName "Manages Windows Filtering Platform (WFP) ALE kernel layers for BandwidthX."
    Write-Host "Service installed successfully." -ForegroundColor Green
}
elseif ($Uninstall) {
    Write-Host "Uninstalling BandwidthX Service..." -ForegroundColor Yellow
    sc.exe stop $serviceName
    sc.exe delete $serviceName
    Write-Host "Service uninstalled." -ForegroundColor Green
}
elseif ($Start) {
    Write-Host "Starting BandwidthX Service..." -ForegroundColor Cyan
    sc.exe start $serviceName
}
elseif ($Stop) {
    Write-Host "Stopping BandwidthX Service..." -ForegroundColor Yellow
    sc.exe stop $serviceName
}
else {
    Write-Host "Usage:"
    Write-Host "  .\install-service.ps1 -Install   (Installs the service)"
    Write-Host "  .\install-service.ps1 -Uninstall (Removes the service)"
    Write-Host "  .\install-service.ps1 -Start     (Starts the service)"
    Write-Host "  .\install-service.ps1 -Stop      (Stops the service)"
}
