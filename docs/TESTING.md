# BandwidthX — Verification & Testing Protocol

This document outlines the testing protocol to verify BandwidthX on Windows 10 and Windows 11 systems.

---

## 1. Automated Integration Test Suite

BandwidthX includes a comprehensive suite of 11 native integration and unit tests covering WFP policy rules, IPC framing, process attribution, PID reuse protection, and SQLite persistence.

### Running the Test Suite:
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd C:\Users\tanu\BandwidthX
cargo test --workspace
```

### Automated Test Matrix:

| Test Case | Module | Description | Result |
|---|---|---|---|
| `test_query_per_tcp_estats` | `bandwidthx-app` | Verifies real-time TCP EStats query (`GetPerTcpConnectionEStats`) reading `DataBytesIn` / `DataBytesOut`. | **PASSED** |
| `test_iphelper_live_telemetry` | `bandwidthx-app` | Verifies `GetIfTable2`, `GetExtendedTcpTable`, and `GetExtendedUdpTable` execution on the Windows kernel. | **PASSED** |
| `test_sqlite_rules_and_analytics` | `bandwidthx-app` | Verifies SQLite schema creation, CRUD operations, analytics aggregation, and CSV export. | **PASSED** |
| `test_guid_constants` | `bandwidthx-common` | Verifies WFP Provider, Sublayer, and IPC pipe GUID consistency. | **PASSED** |
| `test_rule_serialization` | `bandwidthx-common` | Verifies JSON serialization and deserialization across IPC boundaries. | **PASSED** |
| `test_default_exclusive_state` | `bandwidthx-common` | Verifies default state and system process exemption list (`svchost.exe`, `System`, `lsass.exe`, `services.exe`). | **PASSED** |
| `test_chrome_vs_firefox_policy_matrix` | `bandwidthx-common` | Verifies bidirectional isolation matrix (Chrome allowed vs Firefox blocked, and inverse). | **PASSED** |
| `test_emergency_restore_and_sublayer_contract` | `bandwidthx-common` | Verifies sublayer cleanup and filter flush on emergency rollback. | **PASSED** |
| `test_loopback_and_dns_exemption_contract` | `bandwidthx-common` | Verifies loopback (`127.0.0.1`/`::1`) and essential service exemption contract. | **PASSED** |
| `test_ipc_framing_and_bounds` | `bandwidthx-common` | Verifies 4-byte big-endian length prefix framing, decode validity, and oversized payload rejection. | **PASSED** |
| `test_pid_reuse_lifetime_invalidation` | `bandwidthx-common` | Verifies kernel creation timestamp (`GetProcessTimes`) comparison for flushing stale metrics upon PID recycling. | **PASSED** |

---

## 2. Interactive Verification Protocols (Windows 10 / 11)

### Test Scenario 1: Chrome Allowed, Firefox Blocked (Exclusive Mode)
1. Launch two different web browsers (e.g. Google Chrome and Mozilla Firefox).
2. Start BandwidthX:
   ```powershell
   npm run tauri dev
   ```
3. Navigate to **Exclusive Mode**.
4. In the target dropdown, select **Google Chrome** (`chrome.exe`).
5. Click **Enable Exclusive Mode**.
6. **Verification Steps**:
   - In **Google Chrome**: Open `https://speedtest.net` or stream a video on YouTube. Confirm full speed and uninterrupted connectivity.
   - In **Mozilla Firefox**: Attempt to navigate to any website (e.g. `https://wikipedia.org`). Confirm the connection is blocked or times out.
   - Verify that Windows system clock, DNS resolution, and local network devices remain reachable.

### Test Scenario 2: Policy Inversion (Firefox Allowed, Chrome Blocked)
1. With Exclusive Mode active, select **Mozilla Firefox** (`firefox.exe`) as the new target.
2. Click **Apply Policy**.
3. **Verification Steps**:
   - In **Mozilla Firefox**: Refresh `https://wikipedia.org`. Confirm immediate unrestricted access.
   - In **Google Chrome**: Refresh page. Confirm connection is blocked.

### Test Scenario 3: Pre-Existing Connection Teardown
1. In Firefox, begin downloading a large file (or open an active web stream).
2. While the download is actively in progress, select Chrome and enable Exclusive Mode.
3. **Verification Steps**:
   - Confirm that the Firefox download does not continue running in the background.
   - BandwidthX invokes `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)` on Firefox's established sockets, sending immediate TCP RST flags and terminating the flow.

### Test Scenario 4: DNS and DHCP Integrity
1. Enable Exclusive Mode on any test application.
2. Open PowerShell and run:
   ```powershell
   Resolve-DnsName google.com
   Resolve-DnsName cloudflare.com
   ```
3. **Verification Steps**:
   - Confirm DNS queries succeed with valid IP addresses.
   - WFP rule (Remote Port 53, Weight 14) preserves system DNS lookup capabilities.

### Test Scenario 5: Loopback Inter-Process Communication
1. Start a local development server (e.g., `python -m http.server 8080` or `npm run dev`).
2. Enable Exclusive Mode on a non-development application.
3. Open `http://localhost:8080` or `http://127.0.0.1:8080`.
4. **Verification Steps**:
   - Confirm loopback traffic connects normally.
   - WFP rule (`FWP_CONDITION_FLAG_IS_LOOPBACK`, Weight 15) preserves local host networking.

### Test Scenario 6: Emergency One-Click Recovery
1. With Exclusive Mode active and restrictions in place, click the red **Emergency Restore** button in the header bar.
2. **Verification Steps**:
   - Confirm all blocked applications immediately recover connectivity.
   - Confirm active filter count in Settings returns to `0`.

### Test Scenario 7: Dynamic Session Fail-Safe (Crash Simulation)
1. Activate Exclusive Mode.
2. Open Task Manager, locate `bandwidthx-app.exe` (or `bandwidthx-service.exe`), and terminate it via **End Task**.
3. **Verification Steps**:
   - Open any blocked browser.
   - Confirm internet access is instantly restored.
   - Because BandwidthX opens its WFP session with `FWPM_SESSION_FLAG_DYNAMIC`, the Windows kernel destroys all filters the instant the process handle closes.

### Test Scenario 8: Service Named Pipe IPC Security
1. Start `bandwidthx-service.exe` as an administrator.
2. Verify pipe existence using PowerShell:
   ```powershell
   [System.IO.Directory]::GetFiles("\\.\pipe\") | Select-String "BandwidthX"
   ```
3. Confirm the pipe `\\.\pipe\BandwidthX_IPC` is active and handles 4-byte framed IPC requests.
