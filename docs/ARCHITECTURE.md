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
|  |  GetExtendedTcp/Udp |        |  Settings, Audit Log)  |  |
|  |  TCP EStats Data)   |        +------------------------+  |
|  +---------------------+                                    |
|             |                                               |
|  +-------------------------------------------------------+  |
|  |                  IPC Client / Engine                  |  |
|  |   (Framed Named Pipe Client & Local Elevated Fallback)|  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+
                              |
            Named Pipe: \\.\pipe\BandwidthX_IPC
          (4-byte Big-Endian Length-Prefixed Framing)
                              |
+-------------------------------------------------------------+
|            BandwidthX Privileged Windows Service            |
|                                                             |
|  - Process Image Path Validation (QueryFullProcessImage)    |
|  - Windows Filtering Platform (HardenedWfpEngine)           |
|  - Atomic Transactions (Begin0 / Commit0 / Abort0)          |
|  - Explicit Filter Arbitration Weights (15 > 14 > 13 > 1)   |
|  - Established Connection Teardown (MIB_TCP_STATE_DELETE)   |
|  - Dynamic Session Management (FWPM_SESSION_FLAG_DYNAMIC)   |
|  - Essential Service Whitelisting (DNS/DHCP/Loopback)       |
|  - Fail-Safe Emergency Restoration                          |
+-------------------------------------------------------------+
```

---

## 2. Real-Time Telemetry & Metric Attribution

### Proportional Estimation Elimination
Earlier prototypes distributed aggregate interface throughput among running processes based on active socket counts (`process_sockets / total_sockets`). This resulted in severe misattribution: idle applications holding multiple persistent keep-alive sockets appeared to consume significant bandwidth, while high-throughput single-socket streams showed minimal traffic.

BandwidthX eliminates socket-proportional estimation and implements **direct byte-level attribution**:
1. **Interface Throughput**: `GetIfTable2` queries operational network adapters (`IF_TYPE_ETHERNET_CSMACD`, `IF_TYPE_IEEE80211`). Delta calculations across sampling intervals produce genuine total download and upload rates.
2. **TCP Extended Statistics (EStats)**:
   - For active TCP sockets, BandwidthX leverages Windows IP Helper TCP EStats APIs (`GetPerTcpConnectionEStats` / `SetPerTcpConnectionEStats`).
   - By querying `TcpConnectionEstatsData` (`TCP_ESTATS_DATA_ROD_v0`), the engine tracks actual `DataBytesIn` and `DataBytesOut` per socket connection.
3. **Dual IPv4 and IPv6 Polling**:
   - TCP: `GetExtendedTcpTable` polls `AF_INET` (`TCP_TABLE_OWNER_PID_ALL`) and `AF_INET6` (`TCP_TABLE_OWNER_PID_ALL`).
   - UDP: `GetExtendedUdpTable` polls `AF_INET` (`UDP_TABLE_OWNER_PID`) and `AF_INET6` (`UDP_TABLE_OWNER_PID`).
4. **PID Reuse Protection**:
   - Operating systems recycle Process IDs rapidly. If an application exits and a new process inherits its PID, stale metrics could be misattributed.
   - BandwidthX queries kernel process creation timestamps via `GetProcessTimes`. When a PID's creation timestamp changes, the process tracker invalidates stale metrics and reinitializes socket tracking for the new process image.
5. **Inaccessible Process Handling**:
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
  // Add Loopback, DNS, DHCP, Target Permit, and Block Filters...
  FwpmTransactionCommit0(engine);
  ```
- If any filter installation fails (e.g., due to invalid binary path or privilege issue), `FwpmTransactionAbort0` immediately rolls back the transaction with zero side effects.

### Explicit Filter Arbitration & Weights
Filters without explicit weights are arbitrated non-deterministically by WFP. BandwidthX assigns strict numerical weights to ensure predictable evaluation:

| Filter Type | Layer | Match Condition | Action | Weight |
|---|---|---|---|---|
| **Target Application** | ALE Connect / Recv-Accept | `FWPM_CONDITION_ALE_APP_ID` == target app blob | `FWP_ACTION_PERMIT` | `15` |
| **Local Loopback** | ALE Connect / Recv-Accept | `FWPM_CONDITION_FLAGS` & `FWP_CONDITION_FLAG_IS_LOOPBACK` | `FWP_ACTION_PERMIT` | `15` |
| **DNS Resolution** | ALE Connect / Recv-Accept | `FWPM_CONDITION_IP_REMOTE_PORT` == 53 | `FWP_ACTION_PERMIT` | `14` |
| **DHCP Leases** | ALE Connect / Recv-Accept | `FWPM_CONDITION_IP_REMOTE_PORT` in (67, 68, 546, 547) | `FWP_ACTION_PERMIT` | `13` |
| **Default Restriction** | ALE Connect / Recv-Accept | All other applications | `FWP_ACTION_BLOCK` | `1` |

Because Permit rules have higher weights (13 to 15) than the Restrict rule (1), allowed traffic passes unhindered while non-target applications are blocked.

### Active Established Connection Teardown
ALE authorization filters evaluate new connection handshakes (`AUTH_CONNECT` / `AUTH_RECV_ACCEPT`). However, already-established TCP connections bypass ALE connect authorization.

To prevent background applications from continuing to download or upload on existing streams:
- Upon activating Exclusive Mode, BandwidthX enumerates all active TCP entries via `GetExtendedTcpTable`.
- Sockets belonging to unpermitted processes are reset by calling `SetTcpEntry` with:
  ```rust
  row.dwState = MIB_TCP_STATE_DELETE_TCB; // 12
  SetTcpEntry(&row);
  ```
- This forces an immediate TCP RST packet, terminating the flow at the kernel level.

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
- **Security**: Restricted to `NT AUTHORITY\LocalSystem` and `BUILTIN\Administrators`.
- **Framing Protocol**: Messages use a robust 4-byte big-endian length prefix:
  ```
  +-------------------------+----------------------------------+
  | Length Prefix (4 bytes) | JSON Payload (UTF-8 Encoded)     |
  +-------------------------+----------------------------------+
  ```
  - Maximum frame size is strictly capped at 1 MB (`MAX_IPC_FRAME_SIZE`) to prevent memory exhaustion from malformed requests.

### Client Authentication & Verification
- The service does not blindly trust process paths or IDs sent over IPC.
- Upon receiving an Exclusive Mode target request, the service calls `QueryFullProcessImageNameW` on the target PID in kernel mode to independently verify that the executable path matches the requested image.

---

## 6. Technical Boundaries & Rate Limiting Disclosure

- **WFP ALE Enforcement**: BandwidthX utilizes user-space WFP ALE layers. ALE manages socket authorization (permit vs block).
- **Rate Limit Policy**: Processes configured with download/upload bandwidth limits trigger connection-level throttling and selective blocking when exceeding their configured threshold.
- **Traffic Shaping**: True packet-level transmission pacing (token bucket packet delay) requires a kernel callout driver (WFP stream / transport callout) or Windows QoS packet schedulers (`TC API` / `qwave.dll`). BandwidthX documents this distinction clearly to ensure full transparency.
