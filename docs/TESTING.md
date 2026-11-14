# BandwidthX — Verification & Testing Protocol

This document outlines the testing protocols used to verify BandwidthX on Windows 10 and Windows 11 systems.

---

## 1. Automated Test Suite

BandwidthX includes 16 automated tests categorized into Unit Tests, Contract/Mocked Tests, and Native Windows Integration Tests.

### Running the Test Suite:
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd C:\Users\tanu\BandwidthX
cargo test --workspace
```

### Complete Test Matrix:

| Category | Test Case | Target Crate / Module | Description | Result |
|---|---|---|---|---|
| **Real Windows Integration** | `test_query_per_tcp_estats` | `bandwidthx-app::tests::test_estats` | Queries live Windows IP Helper TCP EStats (`GetPerTcpConnectionEStats`) for actual byte metrics. | **PASSED** |
| **Real Windows Integration** | `test_iphelper_live_telemetry` | `bandwidthx-app::tests::backend_tests` | Executes live `GetIfTable2`, `GetExtendedTcpTable`, and `GetExtendedUdpTable` queries on running OS. | **PASSED** |
| **Unit Test** | `test_sqlite_rules_and_analytics` | `bandwidthx-app::tests::backend_tests` | Validates SQLite table schema initialization, CRUD operations, rollup queries, and CSV export. | **PASSED** |
| **Unit Test** | `test_guid_constants` | `bandwidthx-common::tests::common_tests` | Validates deterministic GUID representations for WFP Provider, Sublayer, and IPC Pipe. | **PASSED** |
| **Unit Test** | `test_rule_serialization` | `bandwidthx-common::tests::common_tests` | Tests JSON serialization/deserialization across IPC boundaries for policy models. | **PASSED** |
| **Unit Test** | `test_default_exclusive_state` | `bandwidthx-common::tests::common_tests` | Verifies default state and system process exemption baseline. | **PASSED** |
| **Contract / Mocked** | `test_chrome_vs_firefox_policy_matrix` | `bandwidthx-common::tests::test_enforcement_suite` | Validates bidirectional isolation logic: Chrome permitted vs Firefox blocked, and inverse. | **PASSED** |
| **Contract / Mocked** | `test_dhcpv4_and_dhcpv6_ports_coverage` | `bandwidthx-common::tests::test_enforcement_suite` | Confirms ports 67, 68 (DHCPv4) and 546, 547 (DHCPv6) are in exemption filter matrix. | **PASSED** |
| **Contract / Mocked** | `test_emergency_restore_and_sublayer_contract` | `bandwidthx-common::tests::test_enforcement_suite` | Validates transactional sublayer cleanup and complete filter flush on emergency rollback. | **PASSED** |
| **Contract / Mocked** | `test_loopback_and_dns_exemption_contract` | `bandwidthx-common::tests::test_enforcement_suite` | Verifies loopback (`127.0.0.1`/`::1`) and DNS port 53 permits are prioritized at weights 14-15. | **PASSED** |
| **Contract / Mocked** | `test_ipc_framing_and_bounds` | `bandwidthx-common::tests::test_enforcement_suite` | Validates 4-byte big-endian framing, decode validity, and rejection of oversized frames (>1 MB). | **PASSED** |
| **Contract / Mocked** | `test_pid_reuse_lifetime_invalidation` | `bandwidthx-common::tests::test_enforcement_suite` | Validates kernel process creation timestamp comparison to reject recycled PIDs. | **PASSED** |
| **Contract / Mocked** | `test_process_identity_validation_contract` | `bandwidthx-common::tests::test_enforcement_suite` | Validates rejection of kernel PIDs (0, 4), dead processes, and image path mismatches. | **PASSED** |
| **Contract / Mocked** | `test_rate_limit_policy_distinction` | `bandwidthx-common::tests::test_enforcement_suite` | Verifies distinction between binary ALE authorization and informational rate limit profiles. | **PASSED** |
| **Contract / Mocked** | `test_named_pipe_sddl_generation` | `bandwidthx-common::tests::test_enforcement_suite` | Validates Win32 SDDL string `D:(A;;GA;;;BA)(A;;GA;;;SY)` restricting pipe to Admins and SYSTEM. | **PASSED** |
| **Real Windows Integration** | `test_target_connection_survival_contract` | `bandwidthx-common::tests::test_enforcement_suite` | Executes live socket teardown enumeration, verifying target application connections survive. | **PASSED** |

---

## 2. Interactive Verification Protocols (Windows 10 / 11)

These interactive verification protocols must be performed manually on an active Windows desktop with administrative privileges.

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

### Test Scenario 3: Pre-Existing Connection Teardown & Target Connection Survival
1. In Firefox, begin downloading a large test file (e.g., Ubuntu ISO).
2. In Chrome, start streaming a YouTube video.
3. While both streams are actively in progress, select Chrome and enable Exclusive Mode.
4. **Verification Steps**:
   - **Chrome Stream**: Continues playing without interruption; existing TCP sockets are explicitly preserved.
   - **Firefox Download**: Terminates immediately; BandwidthX invokes `SetTcpEntry(MIB_TCP_STATE_DELETE_TCB)` on Firefox's sockets, sending immediate TCP RST flags and resetting the flow.

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
   - Verify DHCPv4 (67/68) and DHCPv6 (546/547) permit filters remain active.

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
   - Confirm active filter count returns to `0`.

### Test Scenario 7: Dynamic Session Fail-Safe (Crash Simulation)
1. Activate Exclusive Mode.
2. Open Task Manager, locate `bandwidthx-app.exe` (or `bandwidthx-service.exe`), and terminate it via **End Task**.
3. **Verification Steps**:
   - Open any blocked browser.
   - Confirm internet access is instantly restored.
   - Because BandwidthX opens its WFP session with `FWPM_SESSION_FLAG_DYNAMIC`, the Windows kernel destroys all filters the instant the process handle closes.

### Test Scenario 8: Privileged Service Named Pipe Security & Client Elevation
1. Start `bandwidthx-service.exe` as an administrator.
2. Attempt to connect to `\\.\pipe\BandwidthX_IPC` from a non-elevated process or standard user account.
3. **Verification Steps**:
   - Confirm connection is rejected by the OS security subsystem (`ERROR_ACCESS_DENIED` / code 5) via the SDDL DACL `D:(A;;GA;;;BA)(A;;GA;;;SY)`.
   - Confirm elevated client connects successfully and is verified via `TokenElevation`.
