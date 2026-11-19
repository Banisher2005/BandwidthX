# BandwidthX 3.0.0 — Windows Release

**BandwidthX 3.0.0** is a major release introducing a complete glassmorphic redesign inspired by modern Windows 11 utilities, dual-stack per-process network telemetry without simulation, a Fast.com-style internet speed test engine, native Windows services management with safety classifications, safe process termination controls, and kernel-enforced **Exclusive Bandwidth Mode**.

---

## What's New in 3.0.0

### 1. Network: Fast.com-Inspired Speed Test & Live Monitor
- **Fast.com Speed Test Engine**: Real streaming chunk download and buffer upload measurement, accompanied by 10 repeated ping requests to calculate mean latency and jitter (Mean Absolute Deviation of consecutive differences).
- **Dominant Typography**: Prominent real-time speed display with instant progress feedback, cancellation support, and rerun capability.
- **Independent Live Network Monitor**: Always-on telemetry queried directly from Windows IP Helper counters via a centralized telemetry hub, featuring a live 60-second rolling throughput chart.

### 2. Applications: Real Process Telemetry & Granular Control
- **Dual-Stack TCP Extended Statistics**: Process network throughput derived from actual kernel counters via `GetPerTcpConnectionEStats` and `GetPerTcp6ConnectionEStats` (IPv4 & IPv6), completely eliminating socket-count proportional distribution.
- **Independent Block & Allow**: Dedicated controls to block or permit individual applications at the Windows Filtering Platform ALE kernel layer.
- **Protected Process Termination**: Terminate misbehaving applications with kernel process creation timestamp validation (`GetProcessTimes`), preventing race conditions and PID recycling attacks. Core system infrastructure (`System`, `Idle`) is strictly protected.

### 3. Windows Services: Native Classification & Safe Management
- **Win32 SCM Discovery**: Full service enumeration through native Windows Service Control Manager APIs (`EnumServicesStatusExW`, `QueryServiceConfigW`).
- **Safety Classifications**: Services are categorized as **Protected**, **System**, **User**, or **Unknown**. Critical Windows networking services (`RpcSs`, `DcomLaunch`, `BFE`, `mpssvc`, `WinDefend`, `LanmanServer`, `Dnscache`, `Dhcp`) are permanently locked against accidental stoppage.
- **Guarded Lifecycle Controls**: Start, Stop, and Restart controls through native SCM handles.

### 4. Exclusive Bandwidth Mode
- **Dedicated Application Throughput**: Prioritize a game, streaming app, or critical workflow. All other applications are restricted from network access at the WFP ALE layer.
- **Core System Exemptions**: DNS (Port 53), DHCP (Ports 67/68, 546/547), Loopback (`127.0.0.1`, `::1`), and essential Windows services (`svchost.exe`, `lsass.exe`, `services.exe`) are preserved.
- **Active Connection Teardown**: Unpermitted established TCP connections are reset via `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)` while preserving the target application's active connections.
- **Dynamic Fail-Safe Guarantee**: All kernel filters are bound to `FWPM_SESSION_FLAG_DYNAMIC`—if the app closes, crashes, or the PC reboots, Windows flushes all filters automatically.
- **Emergency One-Click Rollback**: Instantly purge all BandwidthX filters and restore default unrestricted networking.

### 5. Centralized Telemetry Hub
- **Eliminated Sub-Interval Wipeouts**: A unified sampling manager caches dual-stack TCP EStats snapshots for 800ms, preventing delta wipeouts caused by concurrent telemetry queries.

---

## Installation Instructions

1. Download **`BandwidthX_3.0.0_x64-setup.exe`** below.
2. Run the installer and approve the Windows UAC elevation prompt (BandwidthX installs per-machine to configure Windows Filtering Platform policies).
3. The installer automatically registers and starts the privileged **BandwidthX** Windows service.
4. Launch BandwidthX from your Start Menu or Desktop.

### Uninstallation
To uninstall, navigate to **Windows Settings → Apps → Installed apps → BandwidthX → Uninstall**. The uninstaller gracefully stops and removes the BandwidthX Windows service before deleting application files.

---

## Release Assets & Checksums

| Asset | Description |
| :--- | :--- |
| **`BandwidthX_3.0.0_x64-setup.exe`** | Official Windows Installer (NSIS, includes desktop app and service) |
| **`BandwidthX_3.0.0_x64-setup.exe.sha256`** | SHA-256 Checksum for the installer |
| **`SHA256SUMS.txt`** | Checksum verification file |

---

## Verification

To verify the integrity of your download in PowerShell:
```powershell
Get-FileHash -Path BandwidthX_3.0.0_x64-setup.exe -Algorithm SHA256
```
Compare the output against `BandwidthX_3.0.0_x64-setup.exe.sha256`.
