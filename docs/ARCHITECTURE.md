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
          (SDDL DACL: LocalSystem + Builtin Admin Only)
          (4-byte Big-Endian Length-Prefixed Framing)
                              |
+-------------------------------------------------------------+
|            BandwidthX Privileged Windows Service            |
|                                                             |
|  - Client Elevation Auth (TokenElevation & Caller PID)      |
|  - Process Validation (QueryFullProcessImage & ProcessTimes)|
|  - Windows Filtering Platform (HardenedWfpEngine)           |
|  - Atomic Transactions (Begin0 / Commit0 / Abort0)          |
|  - Explicit Weights (15 > 14 > 13 > 10 > 1)                 |
|  - Connection Teardown (Target & Exempt Protected)          |
|  - Dynamic Session Management (FWPM_SESSION_FLAG_DYNAMIC)   |
|  - Essential Service Whitelisting (DNS/DHCP/Core Services)  |
|  - Fail-Safe Emergency Restoration                          |
|  - Windows SCM CLI Lifecycle (--install/--start/--status)   |
+-------------------------------------------------------------+
```

---

## 2. Real-Time Telemetry & Metric Attribution

### Proportional Estimation Elimination
Earlier prototypes distributed aggregate interface throughput among running processes based on active socket counts (`process_sockets / total_sockets`). This resulted in severe misattribution: idle applications holding multiple persistent keep-alive sockets appeared to consume significant bandwidth, while high-throughput single-socket streams showed minimal traffic.

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
   - Protected system processes (e.g. anti-cheat or DRM-protected software returning `ERROR_ACCESS_DENIED` / code 5) are gracefully handled using `PROCESS_QUERY_LIMITED_INFORMATION`, ensuring continuous telemetry without application crashes.

---

## 3. Hardened Windows Filtering Platform (WFP) Engine

### ALE Authorization Layers
Network filtering is enforced at the Windows Filtering Platform Application Layer Enforcement (ALE) kernel layers:
- `FWPM_LAYER_ALE_AUTH_CONNECT_V4` (0xC380465D-2271-4C2E-8F46-77433E83594C)
- `FWPM_LAYER_ALE_AUTH_CONNECT_V6` (0x4A72393B-319F-44BC-84C3-BA54DCB3B6B4)
- `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4` (0xE1CD698D-8BE4-4B2E-9C80-5C2C40B8F4F0)
- `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6` (0xA3B42C24-B6E9-4720-B757-0B867252D3F3)

### Transactional Atomicity
To avoid intermediate inconsistent filtering states where legitimate traffic might be dropped during policy reconfiguration:
- All filter additions and deletions are wrapped in kernel transactions:
  ```c
  FwpmTransactionBegin0(engine, 0);
  // Add Loopback, DNS, DHCP, Core Services, Target Permit, and Block Filters...
  FwpmTransactionCommit0(engine);
  ```
- If any filter installation fails (e.g., due to invalid binary path or privilege issue), `FwpmTransactionAbort0` immediately rolls back the transaction with zero side effects.

### Explicit Filter Arbitration & Weights
Filters without explicit weights are arbitrated non-deterministically by WFP. BandwidthX assigns strict numerical weights to ensure predictable evaluation:

| Filter Type | Layer | Match Condition | Action | Weight |
|---|---|---|---|---|
| **Target Application** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_ALE_APP_ID` == target app blob | `FWP_ACTION_PERMIT` | `15` |
| **Local Loopback** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_FLAGS` & `FWP_CONDITION_FLAG_IS_LOOPBACK` | `FWP_ACTION_PERMIT` | `15` |
| **DNS Resolution** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_IP_REMOTE_PORT` == 53 | `FWP_ACTION_PERMIT` | `14` |
| **DHCP Leases** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_IP_REMOTE_PORT` in (67, 68, 546, 547) | `FWP_ACTION_PERMIT` | `13` |
| **Core System Services** | ALE Connect / Recv-Accept (v4 & v6) | `FWPM_CONDITION_ALE_APP_ID` in (`svchost`, `lsass`, `services`) | `FWP_ACTION_PERMIT` | `10` |
| **Default Restriction** | ALE Connect / Recv-Accept (v4 & v6) | All other applications | `FWP_ACTION_BLOCK` | `1` |

Because Permit rules have higher weights (10 to 15) than the Restrict rule (1), allowed traffic passes unhindered while non-target applications are blocked.

### Active Established Connection Teardown
ALE authorization filters evaluate new connection handshakes (`AUTH_CONNECT` / `AUTH_RECV_ACCEPT`). However, already-established TCP connections bypass ALE connect authorization.

To prevent background applications from continuing to download or upload on existing streams:
- Upon activating Exclusive Mode, BandwidthX enumerates all active TCP entries via `GetExtendedTcpTable`.
- **Target Application Protection**: Sockets owned by the chosen target application (`permitted_pid`) are **strictly excluded** from teardown, guaranteeing ongoing sessions survive policy activation without interruption.
- **System Exempt Protection**: System processes (`svchost.exe`, `lsass.exe`, `services.exe`) and kernel PIDs (0, 4) are excluded.
- Sockets belonging to unpermitted processes are reset by calling `SetTcpEntry` with:
  ```rust
  row.dwState = MIB_TCP_STATE_DELETE_TCB; // 12
  SetTcpEntry(&row);
  ```
- This forces an immediate TCP RST packet, terminating unpermitted flows at the kernel level.
- Unpermitted IPv6 sockets are actively inspected and tracked.

---

## 4. Dynamic Session Safety Guarantee (`FWPM_SESSION_FLAG_DYNAMIC`)

Networking tools must guarantee that user network connectivity can never be permanently damaged by software failures.

BandwidthX enforces dynamic kernel sessions:
1. `FwpmEngineOpen0` is always initialized with:
   ```c
   session.flags = FWPM_SESSION_FLAG_DYNAMIC;
   ```
2. Dynamic sessions instruct the Windows kernel to bind all sublayers, filters, and state to the lifetime of the session handle.
3. If the BandwidthX process or service crashes, terminates unexpectedly, or if the machine is powered off, the Windows kernel **automatically purges all BandwidthX filters immediately**.
4. A prominent red **Emergency Restore** action is available across all views to immediately close the session and restore standard networking.

---

## 5. Privileged Service & Named Pipe IPC

### Architecture & Security
- **Pipe Address**: `\\.\pipe\BandwidthX_IPC`
- **Security Descriptor (SDDL DACL)**: `D:(A;;GA;;;BA)(A;;GA;;;SY)`
  - Explicitly grants Generic All (`GA`) only to `BUILTIN\Administrators` (`BA`) and `NT AUTHORITY\LocalSystem` (`SY`).
  - Blocks all unprivileged standard user processes from establishing connections to the pipe.
- **Framing Protocol**: Messages use a robust 4-byte big-endian length prefix:
  ```
  +-------------------------+----------------------------------+
  | Length Prefix (4 bytes) | JSON Payload (UTF-8 Encoded)     |
  +-------------------------+----------------------------------+
  ```
  - Maximum frame size is strictly capped at 1 MB (`MAX_IPC_FRAME_SIZE`) to prevent memory exhaustion from malformed requests.

### Client Authentication & Target Process Validation
- **Caller Authentication**: On every IPC connection, the service extracts the client PID using `GetNamedPipeClientProcessId`. It opens the client process token via `OpenProcessToken` and inspects `TokenElevation`. Callers lacking administrative privileges are rejected with HTTP 403 / Access Denied.
- **Target Process Validation**: The service independently verifies the claimed process:
  1. Opens process handle with `PROCESS_QUERY_LIMITED_INFORMATION`.
  2. Queries executable path via `QueryFullProcessImageNameW` and verifies path equality.
  3. Queries process creation timestamp via `GetProcessTimes` and verifies it matches within 200ms.
  4. Rejects dead PIDs, recycled PIDs, kernel PIDs (0, 4), and mismatched paths.

### Windows Service Lifecycle (SCM)
The standalone service includes full Windows Service Control Manager integration:
```powershell
bandwidthx-service.exe --install     # Registers auto-start service with SCM
bandwidthx-service.exe --uninstall   # Removes service from SCM
bandwidthx-service.exe --start       # Starts service via SCM
bandwidthx-service.exe --stop        # Stops service via SCM
bandwidthx-service.exe --status      # Queries service status from SCM
bandwidthx-service.exe --version     # Displays version info
```

---

## 6. Technical Enforcement Boundaries & Disclosures

- **Binary Authorization (WFP ALE)**: BandwidthX operates at user-mode WFP ALE layers (`ALE_AUTH_CONNECT` / `ALE_AUTH_RECV_ACCEPT`). ALE performs binary authorization: packets matching the policy are permitted (`FWP_ACTION_PERMIT`) or blocked (`FWP_ACTION_BLOCK`).
- **Rate Limit Policy Profiles**: Rate limit values (KB/s upload/download) configured in the UI are saved as persistent policy profiles.
- **Packet Shaping Distinction**: True rate limiting and packet-level transmission pacing (token bucket, leaky bucket, or bandwidth shaping) requires an NDIS Callout driver or kernel transport callout. BandwidthX clearly identifies rate limits as "LIMIT PROFILE (UNENFORCED)" in the UI to maintain absolute engineering honesty and technical transparency.
