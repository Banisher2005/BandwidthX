# BANDWIDTHX — Comprehensive Engineering Audit & Hardening Report

## Executive Summary

An in-depth source-level audit of the BandwidthX codebase was conducted to evaluate network telemetry accuracy, Windows Filtering Platform (WFP) enforcement correctness, IPC security, process lifetime tracking, and recoverability. 

While the desktop user interface, Tauri 2 workspace configuration, and SQLite persistence are well-structured, the audit revealed critical gaps in process attribution, WFP connection handling, IPC synchronization, and rate-limiting enforcement.

---

## Audit Findings Matrix

| ID | Category | Affected File(s) | Severity | Status |
|---|---|---|---|---|
| **AUDIT-01** | Telemetry | `src-tauri/src/process/manager.rs` | **CRITICAL** | Fixed in Phase 2 |
| **AUDIT-02** | Telemetry | `src-tauri/src/network/iphelper.rs` | **HIGH** | Fixed in Phase 2 |
| **AUDIT-03** | Enforcement | `src-tauri/src/service/wfp_engine.rs`, `windows-service/src/main.rs` | **CRITICAL** | Fixed in Phase 3 |
| **AUDIT-04** | Architecture | `src-tauri/src/service/client.rs` | **HIGH** | Fixed in Phase 4 |
| **AUDIT-05** | Security | `windows-service/src/main.rs` | **HIGH** | Fixed in Phase 4 |
| **AUDIT-06** | Enforcement | `src-tauri/src/service/wfp_engine.rs` | **MEDIUM-HIGH** | Fixed in Phase 3 |
| **AUDIT-07** | Enforcement | `src-tauri/src/service/wfp_engine.rs`, `windows-service/src/main.rs` | **HIGH** | Fixed in Phase 3 |
| **AUDIT-08** | Process Identity | `src-tauri/src/process/manager.rs` | **MEDIUM** | Fixed in Phase 2 & 4 |
| **AUDIT-09** | Rate Limiting | `src-tauri/src/commands/mod.rs` | **HIGH** | Fixed in Phase 6 |

---

## Detailed Findings & Technical Analysis

### AUDIT-01: Proportional Socket-Count Estimation of Bandwidth
- **File**: `src-tauri/src/process/manager.rs:93-102`
- **Finding**: Process download and upload speeds were calculated by multiplying global interface throughput by `(process_socket_count / total_socket_count)`.
- **Impact**: Completely inaccurate telemetry. An idle web browser with 40 keep-alive sockets was attributed 80% of network traffic while a single-socket downloading tool was attributed 2%.
- **Remediation**: Transition to actual per-process byte accounting via ETW network tracing (`Microsoft-Windows-Kernel-Network`) and IP Helper connection statistics.

### AUDIT-02: Missing IPv6 Socket Telemetry
- **File**: `src-tauri/src/network/iphelper.rs:135-242`
- **Finding**: `GetExtendedTcpTable` and `GetExtendedUdpTable` only queried `AF_INET` (IPv4).
- **Impact**: Applications communicating over IPv6 (e.g. modern CDNs, Discord, Google services) had zero socket attribution.
- **Remediation**: Query both `AF_INET` and `AF_INET6` for TCP and UDP, unifying sockets by PID.

### AUDIT-03: Existing Connections Left Open After Exclusive Mode / Block Activation
- **File**: `src-tauri/src/service/wfp_engine.rs:147-227`, `windows-service/src/main.rs:94-144`
- **Finding**: WFP ALE layers (`ALE_AUTH_CONNECT` and `ALE_AUTH_RECV_ACCEPT`) only arbitrate **new** connection attempts. Established TCP streams bypass ALE authorization filters.
- **Impact**: When Exclusive Mode was activated, existing downloads, active voice calls, and open streams in background applications continued running.
- **Remediation**: When applying Exclusive Mode or Block policies, enumerate all existing TCP connections belonging to non-exempt processes and forcefully terminate them using `SetTcpEntry` (`MIB_TCP_STATE_DELETE_TCB`).

### AUDIT-04: Disconnected Service and In-Process Engine Duality
- **File**: `src-tauri/src/service/client.rs`
- **Finding**: `ServiceClient` did not connect to the Named Pipe; it called a local in-process `WfpEngine`. Meanwhile, `windows-service` had a separate, unused engine.
- **Impact**: Unprivileged desktop runs failed to enforce WFP, and the architecture maintained two divergent WFP codebases.
- **Remediation**: Unify the WFP engine into `bandwidthx-common` or a shared core, and make `windows-service` the authoritative service with a robust Named Pipe client in Tauri.

### AUDIT-05: Insecure Named Pipe IPC & Unframed Socket Reads
- **File**: `windows-service/src/main.rs:167-218`
- **Finding**: The Named Pipe was created with default security (allowing arbitrary local processes to connect), and IPC messages were read directly into a raw buffer without length-prefix framing.
- **Impact**: Potential local privilege escalation / unauthorized policy changes and message fragmentation risks.
- **Remediation**: Apply explicit Windows Security Descriptors (restricting to Administrators and LocalSystem), implement 4-byte big-endian length prefix framing, and validate client identity.

### AUDIT-06: Auto-Weight Filter Arbitration and Non-Transactional Updates
- **File**: `src-tauri/src/service/wfp_engine.rs:153, 185, 215`
- **Finding**: All WFP filters used auto-weights (`FWP_EMPTY`). No WFP transactions (`FwpmTransactionBegin0`/`Commit0`) were used.
- **Impact**: Unpredictable filter precedence and partial failure states without rollback.
- **Remediation**: Assign deterministic filter weights (Target Permit: 15 > DNS/DHCP Permit: 14 > Loopback: 13 > Block All: 1), and wrap all changes in explicit WFP transactions.

### AUDIT-07: Incomplete Loopback and DHCP Whitelist
- **File**: `src-tauri/src/service/wfp_engine.rs:148-173`
- **Finding**: Loopback and DHCP exemption filters were missing from the actual filter addition code.
- **Impact**: Risk of DHCP lease renewal failure and breakage of local RPC/IPC.
- **Remediation**: Add explicit permit filters for loopback (`127.0.0.1`, `::1`) and DHCP ports (UDP 67/68, 546/547).

### AUDIT-08: PID Reuse Vulnerability
- **File**: `src-tauri/src/process/manager.rs:136-162`
- **Finding**: Process identity tracking was keyed only on PID without creation timestamps.
- **Impact**: Recycled PIDs could be misattributed or mis-targeted.
- **Remediation**: Track process start time using `GetProcessTimes` alongside PID and normalized image path.

### AUDIT-09: Unimplemented Rate Limiting
- **File**: `src-tauri/src/commands/mod.rs:80-87`
- **Finding**: Download and upload limits saved in SQLite were not enforced in the networking engine.
- **Impact**: UI indicated rate limiting, but traffic was either blocked or unrestricted.
- **Remediation**: Clarify WFP ALE capabilities, implement Windows QoS / DSCP / socket throttling where feasible, and clearly disclose technical rate-limiting boundaries.
