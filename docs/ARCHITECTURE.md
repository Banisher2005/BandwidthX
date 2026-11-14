# BandwidthX — Technical Architecture & WFP Engine Design

## 1. System Overview

BandwidthX is a high-performance Windows desktop networking utility engineered for real-time traffic telemetry and process-level bandwidth enforcement. Its defining feature is **Exclusive Mode**, which dedicates the machine's entire network connection to a single selected application while restricting all other user-space processes.

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
|  |  GetExtendedTcp)    |        |  Settings, Audit Log)  |  |
|  +---------------------+        +------------------------+  |
|             |                                               |
|  +-------------------------------------------------------+  |
|  |                 WFP Management Engine                 |  |
|  |    (In-Process Elevated Engine & Service Client)      |  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+
                              |
           Named Pipe: \\.\pipe\BandwidthX_IPC
                              |
+-------------------------------------------------------------+
|            BandwidthX Privileged Windows Service            |
|                                                             |
|  - Windows Filtering Platform (WFP) Management              |
|  - ALE Connect / Recv-Accept Authorization Layers            |
|  - Dynamic Session Management (FWPM_SESSION_FLAG_DYNAMIC)   |
|  - Essential Service Whitelisting (DNS/DHCP/Loopback)       |
|  - Fail-Safe Emergency Restoration                          |
+-------------------------------------------------------------+
```

---

## 2. Windows Filtering Platform (WFP) ALE Enforcement

Network enforcement is implemented directly against the Windows Filtering Platform (WFP) Application Layer Enforcement (ALE) layers.

### ALE Authorization Layers Used:
1. `FWPM_LAYER_ALE_AUTH_CONNECT_V4` (0xC380465D-2271-4C2E-8F46-77433E83594C): Outbound IPv4 connection requests.
2. `FWPM_LAYER_ALE_AUTH_CONNECT_V6` (0x4A72393B-319F-44BC-84C3-BA54Dcb3B6B4): Outbound IPv6 connection requests.
3. `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4` (0xE1CD698D-8BE4-4B2E-9C80-5C2C40B8F4F0): Inbound IPv4 listen and connection acceptance.
4. `FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6` (0xA3B42C24-B6E9-4720-B757-0B867252D3F3): Inbound IPv6 listen and connection acceptance.

### Application Identity Matching
Unlike simple firewalls that filter by volatile Process ID (PID) which can be recycled by the operating system, BandwidthX anchors rules to the normalized NT executable path using:
- `FwpmGetAppIdFromFileName0`: Converts standard Win32 paths (e.g. `C:\Program Files\Google\Chrome\Application\chrome.exe`) into an ALE application blob.
- `FWPM_CONDITION_ALE_APP_ID` with `FWP_MATCH_EQUAL`.

---

## 3. Dynamic Session Safety Guarantee (`FWPM_SESSION_FLAG_DYNAMIC`)

Networking utilities must prioritize recoverability over all else. If an enforcement filter is left behind after an application crash or ungraceful shutdown, user network connectivity could be permanently interrupted.

BandwidthX solves this using Windows Kernel Dynamic Sessions:
1. When connecting to WFP via `FwpmEngineOpen0`, BandwidthX sets `session.flags = FWPM_SESSION_FLAG_DYNAMIC`.
2. Windows kernel registers all sublayers, filters, and providers created in that session to the handle's lifetime.
3. If BandwidthX terminates normally, exits unexpectedly, crashes, or if the PC is restarted, the Windows kernel **instantly and automatically purges all BandwidthX filters**.
4. In addition, an explicit `Emergency Restore` button is accessible from every screen in the UI to immediately destroy filters and revert to standard Windows networking.

---

## 4. Exclusive Mode Whitelist Policy

When Exclusive Mode is activated, BandwidthX installs a dedicated sublayer (`BANDWIDTHX_SUBLAYER_GUID`: `{D0B27163-95D4-4A47-A7EB-29DF34220AA5}`) with weight `0xF000`.

To prevent connection collapse, the following essential traffic is permitted before applying the default restrict rule:
- **Local Loopback**: Sockets to `127.0.0.1` and `::1` remain open for inter-process communication.
- **DNS Resolution**: UDP and TCP traffic to remote port `53` is permitted so the system can resolve hostnames.
- **DHCP Connectivity**: UDP ports `67/68` (IPv4) and `546/547` (IPv6) are preserved for lease renewal.
- **Target Application**: Outbound and inbound traffic matching the selected application's ALE ID is permitted.
- **Restriction Filter**: Any application not matching the above rules is blocked at the BandwidthX sublayer.

---

## 5. Telemetry & IP Helper Architecture

BandwidthX uses genuine Windows networking APIs for monitoring:
- **Interface Throughput**: `GetIfTable2` queries all operational network interfaces (`IF_TYPE_ETHERNET_CSMACD`, `IF_TYPE_IEEE80211`). Delta calculations across sample timestamps compute exact bytes per second without artificial simulation.
- **Socket Attribution**: `GetExtendedTcpTable` (with `TCP_TABLE_OWNER_PID_ALL`) and `GetExtendedUdpTable` (with `UDP_TABLE_OWNER_PID`) poll all active system sockets, their state (`ESTABLISHED`, `LISTEN`, `TIME_WAIT`), and the owning Process ID.
- **Process Resolution**: `QueryFullProcessImageNameW` maps PIDs to full executable paths even for protected system processes using `PROCESS_QUERY_LIMITED_INFORMATION`.

---

## 6. Local IPC Architecture

The UI desktop application communicates with the background privileged service using a named pipe:
- Address: `\\.\pipe\BandwidthX_IPC`
- Security: Local Administrator / LocalSystem DACL.
- Wire Format: JSON RPC protocol using `bandwidthx-common` typed messages (`IpcRequest` and `IpcResponse`).
