# BandwidthX

> **Your Network. Your Rules.**

BandwidthX is a production-quality Windows desktop networking utility engineered for real-time traffic monitoring, process-level telemetry, and kernel-enforced application bandwidth management.

Its defining feature is **EXCLUSIVE MODE**: Select one running application to grant full unrestricted network bandwidth, while systematically restricting other background applications from accessing the internet—both for download and upload traffic—enforced at the Windows Filtering Platform (WFP) kernel layer.

---

## Key Features

- **Exclusive Internet Mode**: Prioritize a game, streaming app, or critical workflow. All other applications are restricted at the WFP ALE layer without disrupting essential system networking.
- **Genuine Live Telemetry**: Direct integration with Windows IP Helper APIs (`GetIfTable2`, `GetExtendedTcpTable`, `GetExtendedUdpTable`) and TCP Extended Statistics (`GetPerTcpConnectionEStats`) for actual per-process throughput without artificial socket-count proportional distribution.
- **Dual IPv4 & IPv6 Coverage**: Full dual-stack monitoring and enforcement across both IPv4 and IPv6 TCP/UDP traffic.
- **Process Identity & PID Reuse Protection**: Verified executable identities via `QueryFullProcessImageNameW` combined with kernel process creation timestamps (`GetProcessTimes`) to prevent stale telemetry and identity confusion upon PID recycling.
- **Active Established Connection Teardown**: Upon Exclusive Mode activation, unpermitted established TCP connections are proactively reset using `SetTcpEntry` with `MIB_TCP_STATE_DELETE_TCB`, ensuring background flows do not leak data after policy enforcement.
- **Dynamic Fail-Safe Guarantee**: Built on `FWPM_SESSION_FLAG_DYNAMIC`. If the application or service ever terminates, crashes, or reboots, the Windows kernel automatically purges all BandwidthX filters immediately.
- **Emergency One-Click Rollback**: Instantly restores default unrestricted networking with a single click from any screen.
- **Authoritative Privileged Service**: Dedicated standalone Windows service communicating via a secure Named Pipe (`\\.\pipe\BandwidthX_IPC`) with length-prefixed framing and independent kernel path verification.
- **Network Rule Engine**: Configure Allow, Block, Limits, and Priority rules anchored to persistent executable identities and backed by SQLite.
- **Network Usage Analytics**: Historical metrics (hourly, daily, weekly, monthly) stored locally in SQLite with interactive charts and CSV data export.
- **Windows 11 Dark Aesthetic**: Designed with deep charcoal surfaces (`#0B0F10`, `#12191A`), subtle green/teal accents (`#91DB79`, `#57C7B5`), and responsive Recharts visualizations.

---

## Architecture & Technology Stack

```
BandwidthX/
├── src/                          # React + TypeScript Frontend
│   ├── components/
│   │   ├── layout/               # Header, Sidebar, Persistent StatusBar
│   │   ├── dashboard/            # Live telemetry cards & 60s rolling AreaChart
│   │   ├── applications/         # Process discovery, search, sorting & rate limit modal
│   │   ├── exclusive/            # The Exclusive Mode hero interface & safeguards
│   │   ├── analytics/            # Historical usage breakdowns & CSV export
│   │   ├── rules/                # Policy engine & custom rule creation
│   │   └── settings/             # Telemetry interval, startup, & diagnostics
│   ├── services/api.ts           # Typed Tauri IPC bridge
│   └── types/index.ts            # Core TypeScript interfaces
│
├── src-tauri/                    # Tauri 2 Desktop Runtime (Rust)
│   ├── src/
│   │   ├── network/iphelper.rs   # GetIfTable2, GetExtendedTcpTable, GetExtendedUdpTable, EStats
│   │   ├── process/manager.rs    # QueryFullProcessImageNameW, EStats aggregation, PID reuse check
│   │   ├── storage/db.rs         # SQLite persistence (rules, analytics, settings)
│   │   ├── service/client.rs     # Framed Named Pipe client with elevated fallback
│   │   ├── commands/mod.rs       # Tauri IPC commands
│   │   └── lib.rs                # App lifecycle, state management, & exit cleanup
│   └── Cargo.toml
│
├── windows-service/              # Standalone Privileged Windows Service
│   ├── src/main.rs               # Framed Named Pipe server (\\.\pipe\BandwidthX_IPC) & kernel verification
│   └── Cargo.toml
│
├── crates/common/                # Shared Core Engine, IPC Framing & WFP Driver Logic
│   ├── src/
│   │   ├── ipc.rs                # 4-byte big-endian length-prefixed binary framing
│   │   ├── wfp.rs                # HardenedWfpEngine (transactions, explicit weights, teardown)
│   │   └── lib.rs                # Protocol types, GUIDs, and IPC message schemas
│   └── Cargo.toml
│
├── docs/                         # Architecture, audit report, and testing documentation
│   ├── ARCHITECTURE.md
│   ├── AUDIT_REPORT.md
│   └── TESTING.md
└── scripts/                      # Build automation and service deployment scripts
```

### Technology Breakdown
- **Desktop Runtime**: Tauri 2 + Rust (`stable-x86_64-pc-windows-msvc`)
- **Frontend**: React 18, TypeScript, Vite, Tailwind CSS, Lucide React, Recharts
- **Networking APIs**: Windows IP Helper (`iphlpapi.dll`), WinSock (`ws2_32.dll`)
- **Kernel Enforcement Layers**: Windows Filtering Platform (`fwpuclnt.dll`):
  - `FWPM_LAYER_ALE_AUTH_CONNECT_V4` & `V6`
  - `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4` & `V6`
- **Application Identification**: `FwpmGetAppIdFromFileName0` with `FWPM_CONDITION_ALE_APP_ID`
- **Local Database**: Embedded SQLite (`rusqlite`)

---

## How Exclusive Mode Works

1. **Target Selection**: The user selects a running application (e.g., `chrome.exe` or `game.exe`).
2. **Kernel Process Validation**: The service queries `QueryFullProcessImageNameW` on the actual kernel process object and validates that the binary path matches the claimed process.
3. **Atomic Transactional WFP Policy**:
   - `FwpmTransactionBegin0` begins an isolated kernel transaction.
   - **Loopback Permit (Weight 15)**: `127.0.0.1` and `::1` remain open via `FWP_CONDITION_FLAG_IS_LOOPBACK`.
   - **DNS Port 53 Permit (Weight 14)**: UDP/TCP port 53 is whitelisted so domain names resolve.
   - **DHCP Port 67/68 & 546/547 Permit (Weight 13)**: DHCP broadcast/unicast is preserved for lease renewal.
   - **Target Permit (Weight 15)**: Outbound and inbound traffic matching the selected application's ALE ID is explicitly allowed.
   - **Default Restrict Filter (Weight 1)**: All other applications in the sublayer are blocked.
   - `FwpmTransactionCommit0` commits the batch atomically. If any call fails, `FwpmTransactionAbort0` reverts state with zero side effects.
4. **Active Connection Teardown**: Sockets belonging to unpermitted applications are reset via `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)` so pre-existing streams cannot bypass the new policy.
5. **Dynamic Kernel Cleanup**: All rules reside under `FWPM_SESSION_FLAG_DYNAMIC`. If the application or service terminates, Windows cleans up immediately.
6. **Instant Reversibility**: Disabling the toggle or clicking Emergency Restore flushes all filters and restores full network access.

---

## Enforcement Boundaries & Rate Limiting Disclosure

- **ALE Authorization**: Windows Filtering Platform ALE (`AUTH_CONNECT` and `AUTH_RECV_ACCEPT`) operates at connection-authorization time. It controls whether sockets can open, connect, or accept data.
- **Connection Rate Limiting**: Applications configured with bandwidth limits trigger connection throttling and policy rejection when exceeding their throughput quota.
- **Packet-Level Traffic Shaping**: Fine-grained per-packet sliding-window transmission pacing requires a dedicated NDIS Callout driver or Windows Quality of Service (QoS) packet schedulers (`qwave.dll` / TC API). BandwidthX explicitly discloses this boundary and does not claim that user-space ALE filters perform packet transmission delay.

---

## Getting Started

### Prerequisites
- Windows 10 or Windows 11 (x64)
- Node.js v18+ and npm
- Rust toolchain (`stable-x86_64-pc-windows-msvc`)
- Visual Studio C++ Build Tools (MSVC with Windows SDK 10/11)

### 1. Install Frontend Dependencies
```powershell
npm install
```

### 2. Run Test Suite
Verify all automated integration tests:
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo test --workspace
```

### 3. Run Development Desktop App
```powershell
npm run tauri dev
```

### 4. Build Production Binaries
```powershell
npm run build
cargo build --release --workspace
```
Compiled executables:
- Desktop Application: `target\release\bandwidthx-app.exe`
- Standalone Service: `target\release\bandwidthx-service.exe`

---

## Installing the Windows Service

To install BandwidthX Privileged Service as a background Windows Service (running as `NT AUTHORITY\LocalSystem`):

```powershell
# Open an elevated Administrator PowerShell prompt:
$serviceExe = "C:\Users\tanu\BandwidthX\target\release\bandwidthx-service.exe"

# 1. Register the service
sc.exe create BandwidthX binPath= "$serviceExe" start= auto DisplayName= "BandwidthX Enforcement Service"

# 2. Start the service
sc.exe start BandwidthX

# 3. Verify status
sc.exe query BandwidthX

# 4. To uninstall
sc.exe stop BandwidthX
sc.exe delete BandwidthX
```

---

## Emergency Recovery Protocol

If network connectivity ever appears blocked or degraded:
1. **Desktop App**: Click the red **Emergency Restore** button in the header or in Settings.
2. **PowerShell (Elevated)**:
   ```powershell
   # Stop the service if running (dynamic sessions will automatically flush)
   sc.exe stop BandwidthX
   
   # Or reset IP stack if needed:
   netsh int ip reset
   netsh winsock reset
   ```
3. **Reboot**: Because all filters are registered under `FWPM_SESSION_FLAG_DYNAMIC`, any reboot completely wipes all active BandwidthX filters at the kernel level.

---

## Safety & Security Design

- **Zero Cloud Reliance**: BandwidthX operates 100% locally. No analytics, telemetry, or configuration data is ever transmitted off the machine.
- **Kernel-Backed Dynamic Sessions**: Enforced with `FWPM_SESSION_FLAG_DYNAMIC`. Filters cannot survive a crash or process termination.
- **Secure Named Pipe IPC**: Communicates over `\\.\pipe\BandwidthX_IPC` with explicit 4-byte framing and independent server-side verification of client process binaries.
- **System Exemptions**: DNS, DHCP, loopback, and essential Windows services (`svchost.exe`, `System`, `lsass.exe`, `services.exe`) are preserved to maintain Windows stability.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
