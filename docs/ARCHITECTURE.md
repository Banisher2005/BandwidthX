# BandwidthX — Technical Architecture & Engine Hardening

## 1. System Overview

BandwidthX is a high-performance Windows desktop networking utility engineered for real-time traffic telemetry and kernel-enforced application bandwidth control. Its defining capability is **Exclusive Mode**, which dedicates the machine's entire network connection to a single selected application while systematically restricting all other user-space processes without destabilizing core Windows networking.

```
+-------------------------------------------------------------+
|                 BandwidthX React Frontend                   |
|     (Vite + TypeScript + Tailwind + Recharts + Lucide)      |
+-------------------------------------------------------------+
                              |
                     Tauri 2 IPC Bridge
                              |
+-------------------------------------------------------------+
|                    BandwidthX Tauri App                     |
|                                                             |
|  +---------------------+        +------------------------+  |
|  |  IP Helper Engine   |        |    SQLite Storage      |  |
|  | (GetIfTable2,       |        | (Rules, Analytics,     |  |
|  |  Dual IPv4/IPv6     |        |  Settings, Audit Log)  |  |
|  |  TCP EStats Data)   |        +------------------------+  |
|  +---------------------+                                    |
|             |                                               |
|  +-------------------------------------------------------+  |
|  |         Authoritative Service IPC Client              |  |
|  |  (Framed Named Pipe Client — No Silent Fallbacks)     |  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+
                              |
            Named Pipe: \\.\pipe\BandwidthX_IPC
          (Fail-Closed SDDL: LocalSystem + Builtin Admin)
          (Verified DACL Kernel Descriptor)
          (4-byte Big-Endian Length-Prefixed Framing)
                              |
+-------------------------------------------------------------+
|            BandwidthX Privileged Windows Service            |
|                                                             |
|  - Windows SCM Dispatcher (StartServiceCtrlDispatcherW)     |
|  - ServiceMain & RegisterServiceCtrlHandlerExW              |
|  - State Transitions (START_PENDING -> RUNNING -> STOPPED)  |
|  - Service Recovery Actions (SERVICE_CONFIG_FAILURE_ACTIONS)|
|  - Client Impersonation & SID Check (CheckTokenMembership)  |
|  - Client Elevation Auth (TokenElevation & Caller PID)      |
|  - Process Handle Pinning & Revalidation (WaitForSingleObj) |
|  - Windows Filtering Platform (HardenedWfpEngine)           |
|  - Atomic Transactions (Begin0 / Commit0 / Abort0)          |
|  - Explicit Weights (15 > 14 > 13 > 10 > 1)                 |
|  - Connection Teardown (Target & Exempt Protected)          |
|  - Dynamic Session Management (FWPM_SESSION_FLAG_DYNAMIC)   |
|  - Essential Service Whitelisting (DNS/DHCP/Core Services)  |
|  - Fail-Safe Emergency Restoration                          |
|  - SCM Management CLI (--install/--start/--stop/--console)  |
+-------------------------------------------------------------+
```

---

## 2. Windows Service Control Manager (SCM) Dispatcher Architecture

### SCM Dispatcher Integration
The privileged service binary (`bandwidthx-service.exe`) implements the native Windows Service Control Manager dispatcher protocol:
1. **Entrypoint Dispatch**: When started without arguments, `main()` invokes `StartServiceCtrlDispatcherW`.
   - If invoked by the Windows SCM, the dispatcher thread connects to the SCM and calls `service_main`.
   - If invoked interactively from a terminal/console, `StartServiceCtrlDispatcherW` fails with `ERROR_FAILED_SERVICE_CONTROLLER_CONNECT` (1063), and the executable automatically falls back to interactive console mode for development and testing.
2. **ServiceMain Execution**:
   - `service_main` registers the extended control handler via `RegisterServiceCtrlHandlerExW`.
   - Immediately transitions to `SERVICE_START_PENDING`.
   - Initializes a Tokio multi-threaded asynchronous runtime.
   - Transitions to `SERVICE_RUNNING` and accepts `SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN`.
3. **Control Handler & Graceful Teardown**:
   - Upon receiving `SERVICE_CONTROL_STOP` or `SERVICE_CONTROL_SHUTDOWN`, the handler transitions to `SERVICE_STOP_PENDING`.
   - Signals the Tokio async server loop via a broadcast channel.
   - Performs an emergency WFP policy flush (`engine.emergency_restore()`), guaranteeing that controlled shutdowns never leave the network filtered.
   - Transitions to `SERVICE_STOPPED` and terminates cleanly.
4. **Service Failure Recovery Actions**:
   - During service registration (`--install`), `ChangeServiceConfig2W` configures `SERVICE_CONFIG_FAILURE_ACTIONS`:
     - 1st Failure: Automatic restart after 5,000ms.
     - 2nd Failure: Automatic restart after 10,000ms.
     - Reset Period: 86,400s (24 hours).

---

## 3. Real-Time Telemetry & Metric Attribution

### Direct Byte-Level Attribution
BandwidthX eliminates socket-proportional estimation and implements **direct byte-level attribution**:
1. **Interface Throughput**: `GetIfTable2` queries operational network adapters (`IF_TYPE_ETHERNET_CSMACD`, `IF_TYPE_IEEE80211`). Delta calculations across sampling intervals produce genuine total download and upload rates. Application totals are kept distinct from interface totals.
2. **Dual-Stack TCP Extended Statistics (EStats)**:
   - For active TCP sockets, BandwidthX leverages Windows IP Helper TCP EStats APIs:
     - IPv4: `GetPerTcpConnectionEStats` / `SetPerTcpConnectionEStats`
     - IPv6: `GetPerTcp6ConnectionEStats` / `SetPerTcp6ConnectionEStats`
   - By querying `TcpConnectionEstatsData` (`TCP_ESTATS_DATA_ROD_v0`), the engine tracks actual `DataBytesIn` and `DataBytesOut` per socket connection.
   - Sockets are keyed by `SocketTelemetryKey` supporting both `V4(SocketAddrV4, SocketAddrV4)` and `V6(SocketAddrV6, SocketAddrV6)`.
3. **Socket Cache Eviction**:
   - The TCP EStats cache actively removes closed connections. Any socket absent from the current active TCP snapshot or inactive for >5 seconds is evicted, preventing memory bloat over extended runtimes.
4. **Dual IPv4 and IPv6 Polling**:
   - TCP: `GetExtendedTcpTable` polls `AF_INET` (`TCP_TABLE_OWNER_PID_ALL`) and `AF_INET6` (`TCP_TABLE_OWNER_PID_ALL`).
   - UDP: `GetExtendedUdpTable` polls `AF_INET` (`UDP_TABLE_OWNER_PID`) and `AF_INET6` (`UDP_TABLE_OWNER_PID`).
5. **PID Reuse Protection**:
   - Operating systems recycle Process IDs rapidly. If an application exits and a new process inherits its PID, stale metrics could be misattributed.
   - BandwidthX queries kernel process creation timestamps via `GetProcessTimes`. When a PID's creation timestamp changes, the process tracker invalidates stale metrics and reinitializes socket tracking for the new process image.
6. **Inaccessible Process Handling**:
   - Protected system processes (returning `ERROR_ACCESS_DENIED` / code 5) are gracefully handled using `PROCESS_QUERY_LIMITED_INFORMATION`, ensuring continuous telemetry without application crashes.

---

## 4. Hardened Windows Filtering Platform (WFP) Engine

### ALE Authorization Layers
Network filtering is enforced at the Windows Filtering Platform Application Layer Enforcement (ALE) kernel layers:
- `FWPM_LAYER_ALE_AUTH_CONNECT_V4` (0xC380465D-2271-4C2E-8F46-77433E83594C)
- `FWPM_LAYER_ALE_AUTH_CONNECT_V6` (0x4A72393B-319F-44BC-84C3-BA54DCB3B6B4)
- `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4` (0xE1CD698D-8BE4-4B2E-9C80-5C2C40B8F4F0)
- `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6` (0xA3B42C24-B6E9-4720-B757-0B867252D3F3)

### Transactional Atomicity
- All filter additions and deletions are wrapped in kernel transactions (`FwpmTransactionBegin0` / `Commit0`).
- If any filter installation fails, `FwpmTransactionAbort0` immediately rolls back the transaction with zero side effects.

### Explicit Filter Arbitration & Weights
Filters without explicit weights are arbitrated non-deterministically by WFP. BandwidthX assigns strict numerical weights:

| Filter Type | Layer | Match Condition | Action | Weight |
|---|---|---|---|---|
| **Target Application** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_ALE_APP_ID` == target app blob | `FWP_ACTION_PERMIT` | `15` |
| **Local Loopback** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_FLAGS` & `FWP_CONDITION_FLAG_IS_LOOPBACK` | `FWP_ACTION_PERMIT` | `15` |
| **DNS Resolution** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_IP_REMOTE_PORT` == 53 | `FWP_ACTION_PERMIT` | `14` |
| **DHCP Leases** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_IP_REMOTE_PORT` in (67, 68, 546, 547) | `FWP_ACTION_PERMIT` | `13` |
| **Core System Services** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_ALE_APP_ID` in (`svchost`, `lsass`, `services`) | `FWP_ACTION_PERMIT` | `10` |
| **Default Restriction** | ALE Connect / Recv-Accept (v4 & v6) | All other applications | `FWP_ACTION_BLOCK` | `1` |

### Active Established Connection Teardown
ALE authorization filters evaluate new connection handshakes (`AUTH_CONNECT` / `AUTH_RECV_ACCEPT`). To prevent background applications from continuing on existing streams:
- **Target Application Protection**: Sockets owned by the chosen target application (`permitted_pid`) are **strictly excluded** from teardown, guaranteeing ongoing sessions survive policy activation without interruption.
- **System Exempt Protection**: System processes (`svchost.exe`, `lsass.exe`, `services.exe`) and kernel PIDs (0, 4) are excluded.
- Sockets belonging to unpermitted processes are reset by calling `SetTcpEntry` with `MIB_TCP_STATE_DELETE_TCB` (12).
- Failed teardown operations are logged and surfaced via `TeardownReport`.
- IPv6 Established Connection Boundary: The Windows user-mode IP Helper API lacks `SetTcp6Entry`. While unmanaged IPv6 connections cannot currently be forcefully terminated via IP Helper, ALE filters block all subsequent packet handshakes.

---

## 5. Fail-Closed IPC Security & Process Validation

### Fail-Closed Named Pipe IPC
- **Pipe Address**: `\\.\pipe\BandwidthX_IPC`
- **Security Descriptor (SDDL DACL)**: `D:(A;;GA;;;BA)(A;;GA;;;SY)`
  - Explicitly grants Generic All (`GA`) only to `BUILTIN\Administrators` (`BA`) and `NT AUTHORITY\LocalSystem` (`SY`).
  - No unsecured fallback: service startup immediately aborts if secure pipe creation fails.
  - Pipe handle security descriptor is authoritatively verified via `GetKernelObjectSecurity`.
- **Framing Protocol**: 4-byte big-endian length prefix with a strict 1 MB maximum payload bound.

### Complete Client Authorization
1. Client PID is queried via `GetNamedPipeClientProcessId`.
2. The server invokes `ImpersonateNamedPipeClient` to adopt the client's security context.
3. `CheckTokenMembership` verifies whether the client is a member of the Builtin Administrators group SID (`S-1-5-32-544`).
4. `RevertToSelf()` restores service context.
5. Client process token is inspected for `TokenElevation`. Both checks must succeed.

### Process Identity Lifetime Pinning & Race Prevention
1. Nonzero creation timestamp required (`claimed_creation_time > 0`).
2. Kernel handle is opened with `PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE`.
3. Holding this open `HANDLE` prevents the Windows kernel from destroying the `EPROCESS` block, guaranteeing that the PID cannot be recycled while policy preparation occurs.
4. Creation timestamp and executable path are verified.
5. Immediately prior to WFP commitment, `WaitForSingleObject(handle, 0)` checks that the target process is still running. If the process terminated, policy enforcement is safely aborted.

---

## 6. Technical Enforcement Boundaries & Disclosures

- **Binary Authorization (WFP ALE)**: BandwidthX operates at user-mode WFP ALE layers (`ALE_AUTH_CONNECT` / `ALE_AUTH_RECV_ACCEPT`). ALE performs binary authorization (Permit vs Block).
- **Rate Limit Policy Profiles**: Rate limit values configured in the UI are stored as persistent policy profiles.
- **Packet-Level Traffic Shaping**: Fine-grained per-packet transmission pacing (token bucket or queue shaping) requires a kernel NDIS Callout driver. Rate limits are clearly presented as "LIMIT PROFILE (UNENFORCED)" in the UI to maintain absolute technical transparency.
