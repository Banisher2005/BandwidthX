# BandwidthX 3.0

> **Your Network. Your Rules.**

BandwidthX is a high-performance Windows desktop networking utility engineered for real-time traffic monitoring, process-level telemetry, and kernel-enforced application bandwidth management.

BandwidthX 3.0 delivers a glassmorphic Windows 11 interface, real dual-stack network telemetry, a built-in Fast.com-inspired speed test engine, safe Windows services management, and its defining capability: **EXCLUSIVE MODE**.

---

## Download & Install (Windows)

Download the Windows setup executable from the [latest GitHub Release](https://github.com/Banisher2005/BandwidthX/releases/latest). The installer registers and starts the privileged BandwidthX service automatically; no manual service commands or development toolchain are required. See [docs/INSTALLATION.md](docs/INSTALLATION.md) for installation, UAC, and uninstall details.

---

## The Three Core Experiences

### 1. Network: Fast.com-Inspired Speed Test & Real-Time Monitor
- **Fast.com Speed Test Engine**: Real streaming chunk download and buffer upload measurement, accompanied by 10 repeated ping requests to calculate mean latency and jitter (Mean Absolute Deviation of consecutive differences). Dominant typography displays download throughput in real time, with instant progress, cancel, and rerun controls.
- **Independent Live Network Monitor**: Always-on telemetry queried directly from Windows IP Helper counters via a centralized telemetry hub, with a live 60-second rolling throughput chart.

### 2. Applications: Real Process Telemetry & Granular Control
- **Genuine Process Telemetry**: Direct integration with Windows IP Helper APIs (`GetIfTable2`, `GetExtendedTcpTable`, `GetExtendedUdpTable`) and dual-stack TCP Extended Statistics (`GetPerTcpConnectionEStats` / `GetPerTcp6ConnectionEStats`). Eliminates artificial socket-count proportional distribution and provides actual byte counters.
- **Independent Access Control**: Clear, separate controls to **Block** or **Allow** applications at the ALE layer.
- **Safe Process Termination**: Protected termination controls with kernel process creation timestamp validation (`GetProcessTimes`), preventing race conditions and PID recycling attacks. System-critical applications (`System`, `Idle`) are strictly protected.

### 3. Windows Services: Native Classification & Safe Management
- **Win32 SCM Enumeration**: Authoritative service discovery via `EnumServicesStatusExW` and binary path discovery via `QueryServiceConfigW`.
- **Safety Classifications**: Services are rigorously classified into **Protected** (locked down core infrastructure such as `RpcSs`, `DcomLaunch`, `BFE`, `mpssvc`, `WinDefend`, `LanmanServer`), **System**, **User**, and **Unknown**.
- **Guarded Lifecycle Controls**: Start, Stop, and Restart controls through native SCM handles (`OpenSCManagerW`, `OpenServiceW`, `ControlService`), with critical system services permanently guarded against accidental stoppage.

---

## Exclusive Bandwidth Mode

Dedicate your entire network interface to a single prioritized application. All other background applications are restricted from inbound and outbound traffic at the Windows Filtering Platform ALE kernel layer.

- **Atomic Transactional WFP Policy**: Configured using `FwpmTransactionBegin0` with explicit sublayer weights.
- **System Safeguards**: Port 53 (DNS), Ports 67/68 and 546/547 (DHCPv4/DHCPv6), Loopback (`127.0.0.1` and `::1`), and core Windows services (`svchost.exe`, `lsass.exe`, `services.exe`) are explicitly whitelisted.
- **Active Established Connection Teardown**: Unpermitted established TCP connections are reset via `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)`. The target application's active connections are strictly preserved.
- **Dynamic Session Safety Guarantee**: Built on `FWPM_SESSION_FLAG_DYNAMIC`. If the application closes, crashes, or the PC restarts, Windows automatically purges all BandwidthX filters immediately.
- **Emergency One-Click Rollback**: Available from every screen to immediately flush all filters and restore unrestricted default networking.

---

## Design System

BandwidthX 3.0 introduces a modern, minimal, glassmorphic aesthetic inspired by Windows 11 utilities:
- **Palette**: Deep charcoal backdrop (`#090D12`), elevated glass panels (`#101820` / `rgba(255, 255, 255, 0.03)`), with vibrant green (`#91DB79`), teal (`#57C7B5`), and red (`#EF6B70`) accents.
- **Glassmorphism**: Backdrop blur (`backdrop-blur-xl`), fine borders (`rgba(255, 255, 255, 0.06)`), and subtle depth shadows.
- **Fast.com Dominant Metrics**: Clean, prominent numerical displays with muted unit descriptors.

---

## Architecture & Technology Stack

```
BandwidthX/
├── src/                          # React 18 + TypeScript Frontend
│   ├── components/
│   │   ├── layout/               # Header, Sidebar, Persistent StatusBar
│   │   ├── dashboard/            # NetworkPage: Fast.com speed test & live throughput
│   │   ├── applications/         # ApplicationsPage: Real telemetry, block/allow & terminate
│   │   ├── services/             # WindowsServicesPage: Win32 SCM discovery & safe control
│   │   ├── exclusive/            # ExclusiveModePage: Target selection, safeguards & toggle
│   │   └── settings/             # SettingsPage: Diagnostics, endpoints & emergency restore
│   ├── services/api.ts           # Typed Tauri IPC bridge with browser fallback
│   └── types/index.ts            # TypeScript interfaces & safety classifications
│
├── src-tauri/                    # Tauri 2 Desktop Runtime (Rust)
│   ├── src/
│   │   ├── network/
│   │   │   ├── telemetry.rs      # CentralizedTelemetryManager (cached single-pass sampling)
│   │   │   ├── speedtest.rs      # SpeedTestEngine (latency, jitter, chunked download/upload)
│   │   │   └── iphelper.rs       # Dual-Stack TCP EStats & IP Helper counters
│   │   ├── service/
│   │   │   ├── manager.rs        # WindowsServiceManager (EnumServicesStatusExW & safety rules)
│   │   │   └── client.rs         # Named Pipe IPC client to privileged Windows service
│   │   ├── process/manager.rs    # Safe process termination & creation time validation
│   │   ├── storage/db.rs         # SQLite persistence for rules & analytics
│   │   └── commands/mod.rs       # Tauri IPC command definitions
│   └── Cargo.toml
│
├── windows-service/              # Standalone Privileged Windows Service
│   ├── src/main.rs               # SCM Dispatcher, ServiceMain, dynamic DACL inspection & RAII impersonation
│   └── Cargo.toml
│
└── crates/common/                # Shared Core Engine, IPC Framing & WFP Driver Logic
    ├── src/
    │   ├── ipc.rs                # 4-byte big-endian length-prefixed binary framing
    │   ├── wfp.rs                # HardenedWfpEngine (transactions, explicit weights, teardown)
    │   └── lib.rs                # Protocol types, GUIDs, and IPC message schemas
    └── Cargo.toml
```

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
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo test --workspace --exclude bandwidthx-service
```

### 3. Run Development App
```powershell
npm run tauri dev
```

### 4. Build Production Packages
```powershell
# Build frontend and Rust binaries
npm run build
cargo build --release --workspace

# Build official MSI and NSIS installers
npm run tauri build
```

Generated Release Artifacts:
- **Windows MSI Installer**: `target/release/bundle/msi/BandwidthX_3.0.0_x64_en-US.msi`
- **Windows Setup Installer**: `target/release/bundle/nsis/BandwidthX_3.0.0_x64-setup.exe`
- **Desktop Application**: `target/release/bandwidthx-app.exe`
- **Standalone Service**: `target/release/bandwidthx-service.exe`

---

## Safety & Security Guarantees

- **100% Local**: No external analytics, tracking, or cloud dependencies. Telemetry is collected directly on your machine.
- **Fail-Safe Dynamic Sessions**: All WFP rules are bound to `FWPM_SESSION_FLAG_DYNAMIC`. The Windows kernel automatically flushes them if the process terminates or PC restarts.
- **Authenticated Privileged IPC**: Standalone service communicates over `\\.\pipe\BandwidthX_IPC` protected by an explicit SDDL DACL (`D:(A;;GA;;;BA)(A;;GA;;;SY)`), verified kernel security descriptor, and RAII-guarded client impersonation with token elevation checks.
- **Protected Service Shields**: Critical Windows networking services (`RpcSs`, `BFE`, `mpssvc`, `Dnscache`, `Dhcp`) cannot be terminated or stopped from the interface.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
