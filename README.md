# BandwidthX

> **Your Network. Your Rules.**

BandwidthX is a high-performance Windows desktop networking utility engineered for real-time traffic monitoring, process-level telemetry, and kernel-enforced application bandwidth management.

Its defining feature is **EXCLUSIVE MODE**: Select one running application to grant full unrestricted network bandwidth, while systematically restricting other background applications from accessing the internet—both for download and upload traffic—enforced at the Windows Filtering Platform (WFP) kernel layer.

---

## Key Features

- **Exclusive Internet Mode**: Prioritize a game, streaming app, or critical workflow. All other applications are restricted at the WFP ALE layer without disrupting essential system networking.
- **Genuine Live Telemetry**: Direct integration with Windows IP Helper APIs (`GetIfTable2`, `GetExtendedTcpTable`, `GetExtendedUdpTable`) and dual-stack TCP Extended Statistics (`GetPerTcpConnectionEStats` / `GetPerTcp6ConnectionEStats`) for actual per-process throughput without artificial socket-count proportional distribution.
- **Dual IPv4 & IPv6 Coverage**: Full dual-stack monitoring and enforcement across both IPv4 and IPv6 TCP/UDP traffic.
- **Process Identity & PID Reuse Protection**: Verified executable identities via `QueryFullProcessImageNameW` combined with kernel process creation timestamps (`GetProcessTimes`) to prevent stale telemetry and identity confusion upon PID recycling.
- **Active Established Connection Teardown**: Upon Exclusive Mode activation, unpermitted established TCP connections are reset using `SetTcpEntry` with `MIB_TCP_STATE_DELETE_TCB`. Sockets belonging to the chosen target application are strictly preserved.
- **Dynamic Fail-Safe Guarantee**: Built on `FWPM_SESSION_FLAG_DYNAMIC`. If the application or service ever terminates, crashes, or reboots, the Windows kernel automatically purges all BandwidthX filters immediately.
- **Emergency One-Click Rollback**: Instantly restores default unrestricted networking with a single click from any screen.
- **Authoritative Privileged Service**: Dedicated standalone Windows service communicating via a secure Named Pipe (`\\.\pipe\BandwidthX_IPC`) protected by an explicit SDDL DACL (`D:(A;;GA;;;BA)(A;;GA;;;SY)`) and client elevation verification (`TokenElevation`).
- **Native SCM Lifecycle Management**: Built-in CLI commands for seamless Windows Service registration, start, stop, and status querying.
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
│   │   ├── network/iphelper.rs   # GetIfTable2, GetExtendedTcp/UdpTable, Dual-Stack TCP EStats
│   │   ├── process/manager.rs    # QueryFullProcessImageNameW, EStats aggregation, PID reuse check
│   │   ├── storage/db.rs         # SQLite persistence (rules, analytics, settings)
│   │   ├── service/client.rs     # Framed Named Pipe client (authoritative service connection)
│   │   ├── commands/mod.rs       # Tauri IPC commands
│   │   └── lib.rs                # App lifecycle, state management, & exit cleanup
│   └── Cargo.toml
│
├── windows-service/              # Standalone Privileged Windows Service
│   ├── src/main.rs               # Framed Named Pipe server, SDDL security, SCM CLI, & process validation
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
2. **Kernel Process Validation**: The service independently calls `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, `QueryFullProcessImageNameW`, and `GetProcessTimes` on the running process instance to confirm that the image path matches and that the creation timestamp is valid (preventing PID reuse attacks).
3. **Atomic Transactional WFP Policy**:
   - `FwpmTransactionBegin0` begins an isolated kernel transaction.
   - **Loopback Permit (Weight 15)**: `127.0.0.1` and `::1` remain open via `FWP_CONDITION_FLAG_IS_LOOPBACK`.
   - **DNS Port 53 Permit (Weight 14)**: UDP/TCP port 53 is whitelisted so domain names resolve.
   - **DHCP Port 67/68 & 546/547 Permit (Weight 13)**: DHCPv4 and DHCPv6 broadcast/unicast are preserved for lease renewal.
   - **Core Windows Services Permit (Weight 10)**: `svchost.exe`, `lsass.exe`, and `services.exe` ALE App IDs are permitted.
   - **Target Permit (Weight 15)**: Outbound and inbound traffic matching the selected application's ALE ID is explicitly allowed.
   - **Default Restrict Filter (Weight 1)**: All other applications in the sublayer are blocked.
   - `FwpmTransactionCommit0` commits the batch atomically. If any call fails, `FwpmTransactionAbort0` reverts state with zero side effects.
4. **Active Connection Teardown**: Sockets belonging to unpermitted applications are reset via `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)`. Target application sockets, system exempt sockets, and loopback sockets are strictly preserved.
5. **Dynamic Kernel Cleanup**: All rules reside under `FWPM_SESSION_FLAG_DYNAMIC`. If the application or service terminates, Windows cleans up immediately.
6. **Instant Reversibility**: Disabling the toggle or clicking Emergency Restore flushes all filters and restores full network access.

---

## Enforcement Boundaries & Rate Limiting Disclosure

- **Binary ALE Authorization**: Windows Filtering Platform ALE (`AUTH_CONNECT` and `AUTH_RECV_ACCEPT`) operates at connection-authorization time. It controls whether sockets can open, connect, or accept data (Permit vs Block).
- **Rate Limit Policy Profiles**: Rate limit rules saved in the database are stored as policy profiles. In the UI, they are clearly marked as `LIMIT PROFILE (UNENFORCED)`.
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

## Installing & Managing the Windows Service

The standalone service executable includes built-in commands for the Windows Service Control Manager (SCM):

```powershell
# Open an elevated Administrator PowerShell prompt:
cd C:\Users\tanu\BandwidthX\target\release

# 1. Register the service (Auto-start)
.\bandwidthx-service.exe --install

# 2. Start the service
.\bandwidthx-service.exe --start

# 3. Query service status
.\bandwidthx-service.exe --status

# 4. Stop the service
.\bandwidthx-service.exe --stop

# 5. Uninstall the service
.\bandwidthx-service.exe --uninstall
```

---

## Emergency Recovery Protocol

If network connectivity ever appears blocked or degraded:
1. **Desktop App**: Click the red **Emergency Restore** button in the header or in Settings.
2. **PowerShell (Elevated)**:
   ```powershell
   # Stop the service if running (dynamic sessions will automatically flush)
   .\bandwidthx-service.exe --stop
   
   # Or reset IP stack if needed:
   netsh int ip reset
   netsh winsock reset
   ```
3. **Reboot**: Because all filters are registered under `FWPM_SESSION_FLAG_DYNAMIC`, any reboot completely wipes all active BandwidthX filters at the kernel level.

---

## Safety & Security Design

- **Zero Cloud Reliance**: BandwidthX operates 100% locally. No analytics, telemetry, or configuration data is ever transmitted off the machine.
- **Kernel-Backed Dynamic Sessions**: Enforced with `FWPM_SESSION_FLAG_DYNAMIC`. Filters cannot survive a crash or process termination.
- **Secure Named Pipe IPC**: Communicates over `\\.\pipe\BandwidthX_IPC` protected by an explicit SDDL DACL (`D:(A;;GA;;;BA)(A;;GA;;;SY)`) and client elevation verification (`TokenElevation`).
- **System Exemptions**: DNS, DHCPv4, DHCPv6, loopback, and essential Windows services (`svchost.exe`, `System`, `lsass.exe`, `services.exe`) are preserved to maintain Windows stability.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
