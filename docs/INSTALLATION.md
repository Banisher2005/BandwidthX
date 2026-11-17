# BandwidthX for Windows — Installation

## Download

Download **BandwidthX_3.0.0_x64-setup.exe** from the [latest GitHub Release](https://github.com/Banisher2005/BandwidthX/releases/latest).

## Install

1. Run the setup executable.
2. Approve the Windows UAC prompt. BandwidthX installs per-machine because its privileged Windows service configures Windows Filtering Platform policies.
3. The installer registers and starts the **BandwidthX** Windows service automatically.
4. Launch BandwidthX from the Start Menu. Windows will request elevation when the desktop app starts because the current named-pipe security policy permits Administrators and LocalSystem only.

The installer includes both the desktop application and the Windows service. You do not need to install Rust, Node.js, or run PowerShell commands manually.

## Uninstall

Use **Settings → Apps → Installed apps → BandwidthX → Uninstall**. The uninstaller stops and removes the BandwidthX service before deleting the application files.

## Important notes

- Windows 10/11 x64 is the supported release target.
- The installer is not code-signed. Windows SmartScreen may show an unknown-publisher warning. Only install the setup executable downloaded from the official repository release page.
- BandwidthX uses Windows Filtering Platform ALE authorization. Saved rate-limit profiles are not packet-level bandwidth shaping and are labelled as unenforced in the UI.
- Established IPv6 TCP sessions cannot currently be forcibly torn down through the available user-mode IP Helper API; new policy decisions remain enforced by WFP.
- If networking appears restricted unexpectedly, launch BandwidthX and use **Emergency Restore**, or stop the BandwidthX service from an elevated terminal.

## Developer build

The release workflow builds the service first, stages it as a Tauri resource, then creates the NSIS setup executable. To reproduce locally from an elevated Windows development environment:

```powershell
npm ci
cargo build --release -p bandwidthx-service
New-Item -ItemType Directory -Force src-tauri/resources | Out-Null
Copy-Item target/release/bandwidthx-service.exe src-tauri/resources/bandwidthx-service.exe -Force
npx tauri build --bundles nsis
```

The setup executable is written to target\release\bundle\nsis\.
