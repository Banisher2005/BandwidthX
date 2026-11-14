# BandwidthX

> **Your Network. Your Rules.**

BandwidthX is a production-quality Windows desktop networking utility engineered for real-time traffic monitoring and granular process-level bandwidth management.

Its defining feature is **EXCLUSIVE MODE**: Select one running application to grant full unrestricted network bandwidth, while systematically restricting other background applications from accessing the internet—both for download and upload traffic—enforced at the Windows Filtering Platform (WFP) kernel layer.

---

## Key Features

- **Exclusive Internet Mode**: Prioritize a game, streaming app, or critical workflow. All other applications are restricted at the WFP ALE layer without disrupting essential system networking.
- **Genuine Live Telemetry**: No simulated statistics or fake graphs. Direct integration with Windows IP Helper APIs (`GetIfTable2`, `GetExtendedTcpTable`, `GetExtendedUdpTable`).
- **Process-Level Attribution**: Monitors active TCP/UDP sockets, resolves verified executable paths via `QueryFullProcessImageNameW`, and calculates live throughput.
- **Dynamic Fail-Safe Guarantee**: Built on `FWPM_SESSION_FLAG_DYNAMIC`. If the application or service ever terminates, crashes, or reboots, Windows automatically purges all BandwidthX filters.
- **Emergency One-Click Rollback**: Instantly restores default unrestricted networking with a single click from any screen.
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
│   │   ├── applications/         # Process discovery, search, sorting & actions
│   │   ├── exclusive/            # The Exclusive Mode hero interface & safeguards
│   │   ├── analytics/            # Historical usage breakdowns & CSV export
│   │   ├── rules/                # Policy engine & custom rule creation
│   │   └── settings/             # Telemetry interval, startup, & diagnostics
│   ├── services/api.ts           # Typed Tauri IPC bridge
│   └── types/index.ts            # Core TypeScript interfaces
│
├── src-tauri/                    # Tauri 2 Desktop Runtime (Rust)
│   ├── src/
│   │   ├── network/iphelper.rs   # GetIfTable2, GetExtendedTcpTable, GetExtendedUdpTable
│   │   ├── process/manager.rs    # QueryFullProcessImageNameW & socket aggregation
│   │   ├── storage/db.rs         # SQLite persistence (rules, analytics, settings)
│   │   ├── service/wfp_engine.rs # In-process elevated WFP ALE engine
│   │   ├── commands/mod.rs       # Tauri IPC commands
│   │   └── lib.rs                # App lifecycle, state management, & exit cleanup
│   └── Cargo.toml
│
├── windows-service/              # Standalone Privileged Windows Service
│   ├── src/main.rs               # Named Pipe server (\\.\pipe\BandwidthX_IPC) & WFP ALE
│   └── Cargo.toml
│
├── crates/common/                # Shared protocol, types, & WFP GUID constants
├── database/schema.sql           # Reference SQLite database schema
├── docs/                         # Architecture and testing documentation
└── scripts/                      # Build and service management automation
```

### Technology Breakdown
- **Desktop Runtime**: Tauri 2 + Rust
- **Frontend**: React 18, TypeScript, Vite, Tailwind CSS, Lucide React, Recharts
- **Networking APIs**: Windows IP Helper (`iphlpapi.dll`), WinSock (`ws2_32.dll`)
- **Enforcement Layers**: Windows Filtering Platform (`fwpuclnt.dll`) ALE Layers:
  - `FWPM_LAYER_ALE_AUTH_CONNECT_V4` & `V6`
  - `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4` & `V6`
- **Application Identification**: `FwpmGetAppIdFromFileName0` with `FWPM_CONDITION_ALE_APP_ID`
- **Local Database**: Embedded SQLite (`rusqlite`)

---

## How Exclusive Mode Works

1. **Target Selection**: The user selects a running application (e.g., `chrome.exe` or `game.exe`).
2. **Identity Verification**: The engine retrieves the full binary path and queries the NT application identifier via `FwpmGetAppIdFromFileName0`.
3. **Sublayer Activation**: A high-priority BandwidthX WFP sublayer is initialized within a dynamic kernel session.
4. **Essential Whitelist**:
   - **Loopback**: `127.0.0.1` and `::1` remain open.
   - **DNS**: UDP/TCP port 53 is permitted so hostnames resolve.
   - **DHCP**: UDP ports 67/68 and 546/547 are preserved for IP leases.
   - **Target**: Outbound and inbound traffic matching the selected application's ALE ID is explicitly permitted.
5. **Restriction**: All other user applications are restricted at the BandwidthX sublayer weight.
6. **Instant Reversibility**: Disabling the toggle or clicking Emergency Restore immediately closes the sublayer and restores default routing.

---

## Getting Started

### Prerequisites
- Windows 10 or Windows 11 (x64)
- Node.js v18+ and npm
- Rust toolchain (`stable-x86_64-pc-windows-msvc`)
- Visual Studio C++ Build Tools (MSVC with Windows SDK 10/11)

### 1. Install Dependencies
```powershell
npm install
```

### 2. Run Integration Tests
```powershell
cargo test --workspace
```

### 3. Run Development Mode
```powershell
npm run tauri dev
```

### 4. Build Production Binaries
```powershell
.\scripts\build.ps1
```
The compiled binaries will be output to `target\release\`.

---

## Safety & Security Design

- **Zero Cloud Reliance**: BandwidthX operates 100% locally. No analytics, telemetry, or configuration data is ever transmitted off the machine.
- **Fail-Safe Dynamic Session**: Enforced with `FWPM_SESSION_FLAG_DYNAMIC`. Filters cannot survive a crash or process termination.
- **Emergency Restore**: A prominent recovery button is hardwired into the header and settings to immediately flush all filters.
- **System Protection**: DNS, DHCP, loopback, and essential Windows services (`svchost.exe`, `System`, `lsass.exe`) are whitelisted to prevent system instability.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
