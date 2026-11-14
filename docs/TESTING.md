# BandwidthX — Verification & Testing Protocol

This document outlines the testing procedures to verify BandwidthX on Windows 10 and Windows 11 machines.

---

## 1. Automated Integration Tests

Run the native test suite from PowerShell:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd BandwidthX
cargo test --workspace
```

### Verified Test Cases:
1. `test_default_exclusive_state`: Verifies default exemptions list (`svchost.exe`, `System`, `lsass.exe`, `services.exe`).
2. `test_guid_constants`: Verifies WFP Provider and Sublayer GUID consistency.
3. `test_rule_serialization`: Verifies JSON deserialization across IPC boundaries.
4. `test_sqlite_rules_and_analytics`: Verifies SQLite schema creation, CRUD operations, analytics aggregation, and CSV export.
5. `test_iphelper_live_telemetry`: Verifies real-time `GetIfTable2`, `GetExtendedTcpTable`, and `GetExtendedUdpTable` execution on the Windows kernel.

---

## 2. Interactive Feature Verification on Windows

### Test 1: Real-Time Network Monitoring
1. Launch BandwidthX:
   ```powershell
   npm run tauri dev
   ```
2. Navigate to **Overview**.
3. Verify that the **Download** and **Upload** cards update continuously with actual speeds.
4. Start a network download (e.g., speed test or file download).
5. Confirm that the 60-second rolling AreaChart responds immediately to the bandwidth surge.
6. Press **Pause** on the chart; confirm the telemetry freezes. Press **Resume** to restart.

### Test 2: Process Network Attribution
1. Navigate to **Applications**.
2. Verify that running network applications (e.g., Chrome, Discord, Spotify) appear with:
   - Valid PID
   - Full executable path
   - Live download and upload rates
   - Number of active TCP/UDP sockets
3. Test search filter: type `chrome`; verify instant filtering.
4. Test sorting: click **Download** column header to sort by download speed descending.

### Test 3: Application-Specific Access Control
1. Find a test application (e.g., Spotify or Discord).
2. Click the **Shield / Block** icon.
3. Confirm that the application's status changes to `BLOCKED`.
4. Attempt to stream or reconnect within the application; confirm network access is denied.
5. Click **Unblock**; confirm network connectivity resumes immediately.

### Test 4: Exclusive Mode Enforcement (Core Objective)
1. Navigate to **Exclusive Mode**.
2. Select your browser (e.g., `chrome.exe`).
3. Verify that the target executable path and current speeds display in the target review card.
4. Click **ENABLE EXCLUSIVE MODE**.
5. Observe:
   - Status badge transitions to `EXCLUSIVE MODE: chrome.exe`.
   - `chrome.exe` remains connected to the internet.
   - Other applications (e.g., secondary browser, Discord, game launcher) are restricted from network access.
6. Verify DNS resolution: Navigate to a new website in Chrome; confirm DNS resolves normally due to port 53 whitelisting.
7. Click **DISABLE EXCLUSIVE MODE**; confirm all other applications instantly regain connectivity.

### Test 5: Safe Fail-Safe & Emergency Restore
1. Enable Exclusive Mode or block an application.
2. Click the red **Emergency Restore** button in the top navigation bar or Settings page.
3. Confirm that all BandwidthX filters are instantly purged and normal network access is restored.
4. Test process crash safety:
   - Enable Exclusive Mode.
   - Force-kill BandwidthX from Task Manager (`Stop-Process -Name BandwidthX -Force`).
   - Confirm that Windows kernel immediately deletes all filters because `FWPM_SESSION_FLAG_DYNAMIC` was set.
   - Network connectivity for all applications must remain completely normal.

### Test 6: Network Analytics & CSV Export
1. Navigate to **Network Analytics**.
2. Verify that accumulated traffic is displayed by application and inbound/outbound ratio.
3. Click **Export to CSV**.
4. Open the downloaded `.csv` file in Excel or Notepad; verify headers and recorded byte metrics.
