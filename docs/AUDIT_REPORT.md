# BANDWIDTHX — Comprehensive Engineering Audit & Hardening Report

## Executive Summary

An in-depth source-level audit of the BandwidthX codebase was conducted across versions 2.0, 2.1, and 2.2 to evaluate network telemetry accuracy, Windows Filtering Platform (WFP) enforcement correctness, IPC security, process lifetime tracking, Windows Service Control Manager (SCM) dispatcher compliance, and recoverability. 

While the desktop user interface, Tauri 2 workspace configuration, and SQLite persistence are well-structured, earlier audits revealed critical gaps in process attribution, WFP connection handling, IPC synchronization, SCM lifecycle dispatching, and rate-limiting enforcement.

All identified vulnerabilities and structural defects have been systematically remediated and verified through automated test suites and live Windows integration protocols.

---

## Audit Findings Matrix

| ID | Category | Affected File(s) | Severity | Status |
|---|---|---|---|---|
| **AUDIT-01** | Telemetry | `src-tauri/src/process/manager.rs` | **CRITICAL** | Fixed in v2.0 |
| **AUDIT-02** | Telemetry | `src-tauri/src/network/iphelper.rs` | **HIGH** | Fixed in v2.0 |
| **AUDIT-03** | Enforcement | `src-tauri/src/service/wfp_engine.rs`, `windows-service/src/main.rs` | **CRITICAL** | Fixed in v2.0 |
| **AUDIT-04** | Architecture | `src-tauri/src/service/client.rs` | **HIGH** | Fixed in v2.0 |
| **AUDIT-05** | Security | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.0 |
| **AUDIT-06** | Enforcement | `src-tauri/src/service/wfp_engine.rs` | **MEDIUM-HIGH** | Fixed in v2.0 |
| **AUDIT-07** | Enforcement | `src-tauri/src/service/wfp_engine.rs`, `windows-service/src/main.rs` | **HIGH** | Fixed in v2.0 |
| **AUDIT-08** | Process Identity | `src-tauri/src/process/manager.rs` | **MEDIUM** | Fixed in v2.0 |
| **AUDIT-09** | Rate Limiting | `src-tauri/src/commands/mod.rs` | **HIGH** | Fixed in v2.0 |
| **AUDIT-10** | Process Validation | `windows-service/src/main.rs` | **CRITICAL** | Fixed in v2.1 |
| **AUDIT-11** | Enforcement | `crates/common/src/wfp.rs` | **CRITICAL** | Fixed in v2.1 |
| **AUDIT-12** | Named Pipe Security | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.1 |
| **AUDIT-13** | Single Authority | `src-tauri/src/service/client.rs` | **HIGH** | Fixed in v2.1 |
| **AUDIT-14** | Exemptions | `crates/common/src/wfp.rs` | **MEDIUM-HIGH** | Fixed in v2.1 |
| **AUDIT-15** | Dual-Stack Telemetry | `src-tauri/src/network/iphelper.rs` | **MEDIUM** | Fixed in v2.1 |
| **AUDIT-16** | Feature Disclosure | `src/components/applications/ApplicationsPage.tsx` | **MEDIUM** | Fixed in v2.1 |
| **AUDIT-17** | Service Lifecycle | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.1 |
| **AUDIT-18** | SCM Dispatcher | `windows-service/src/main.rs` | **CRITICAL** | Fixed in v2.2 |
| **AUDIT-19** | IPC Fail-Closed | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.2 |
| **AUDIT-20** | Client Authorization | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.2 |
| **AUDIT-21** | Race Condition & PID Pinning | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.2 |
| **AUDIT-22** | Teardown Reporting | `crates/common/src/wfp.rs` | **MEDIUM** | Fixed in v2.2 |
| **AUDIT-23** | Service Failure Recovery | `windows-service/src/main.rs` | **MEDIUM** | Fixed in v2.2 |
| **AUDIT-24** | Pipe DACL Dynamic Validation | `windows-service/src/main.rs` | **CRITICAL** | Fixed in v2.3 |
| **AUDIT-25** | Impersonation Ordering & RAII | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.3 |
| **AUDIT-26** | Non-Blocking SCM Handler | `windows-service/src/main.rs` | **CRITICAL** | Fixed in v2.3 |
| **AUDIT-27** | SCM Recovery Verification | `windows-service/src/main.rs` | **HIGH** | Fixed in v2.3 |
| **AUDIT-28** | Real SCM Integration Suite | `windows-service/tests/real_windows_scm_integration.rs` | **CRITICAL** | Fixed in v2.3 |

---

## Detailed Findings & Technical Analysis

### AUDIT-01: Proportional Socket-Count Estimation of Bandwidth
- **File**: `src-tauri/src/process/manager.rs:93-102`
- **Finding**: Process download and upload speeds were calculated by multiplying global interface throughput by `(process_socket_count / total_socket_count)`.
- **Impact**: Inaccurate telemetry. An idle web browser with 40 keep-alive sockets was attributed 80% of network traffic while a single-socket downloading tool was attributed 2%.
- **Remediation**: Transitioned to actual per-process byte accounting via IP Helper connection statistics and TCP EStats query (`GetPerTcpConnectionEStats`).

### AUDIT-02: Missing IPv6 Socket Telemetry
- **File**: `src-tauri/src/network/iphelper.rs:135-242`
- **Finding**: `GetExtendedTcpTable` and `GetExtendedUdpTable` only queried `AF_INET` (IPv4).
- **Impact**: Applications communicating over IPv6 had zero socket attribution.
- **Remediation**: Query both `AF_INET` and `AF_INET6` for TCP and UDP, unifying sockets by PID.

### AUDIT-03: Existing Connections Left Open After Exclusive Mode / Block Activation
- **File**: `src-tauri/src/service/wfp_engine.rs:147-227`, `windows-service/src/main.rs:94-144`
- **Finding**: WFP ALE layers (`ALE_AUTH_CONNECT` and `ALE_AUTH_RECV_ACCEPT`) only arbitrate **new** connection attempts. Established TCP streams bypass ALE authorization filters.
- **Impact**: Background applications continued running on pre-existing streams.
- **Remediation**: Forcefully terminate unpermitted existing TCP connections using `SetTcpEntry` (`MIB_TCP_STATE_DELETE_TCB`).

### AUDIT-04: Disconnected Service and In-Process Engine Duality
- **File**: `src-tauri/src/service/client.rs`
- **Finding**: `ServiceClient` called a local in-process `WfpEngine` rather than communicating with the privileged service.
- **Impact**: Unprivileged runs failed silently.
- **Remediation**: Unified WFP engine into `bandwidthx-common` and established the standalone service as single authority.

### AUDIT-05: Insecure Named Pipe IPC & Unframed Socket Reads
- **File**: `windows-service/src/main.rs:167-218`
- **Finding**: Pipe had default security and IPC reads lacked framing bounds.
- **Impact**: Local privilege escalation and message fragmentation risks.
- **Remediation**: Applied explicit SDDL descriptors, 4-byte big-endian framing, and client authorization.

### AUDIT-06: Auto-Weight Filter Arbitration and Non-Transactional Updates
- **File**: `src-tauri/src/service/wfp_engine.rs:153, 185, 215`
- **Finding**: WFP filters used auto-weights and lacked transactional wrapping.
- **Impact**: Non-deterministic evaluation and partial failure states.
- **Remediation**: Explicit weights (15 > 14 > 13 > 10 > 1) and transactional commits (`FwpmTransactionBegin0`/`Commit0`).

### AUDIT-07: Incomplete Loopback and DHCP Whitelist
- **File**: `src-tauri/src/service/wfp_engine.rs:148-173`
- **Finding**: Loopback and DHCP exemption filters were missing from the filter set.
- **Impact**: Risk of DHCP lease renewal failure and breakage of local RPC/IPC.
- **Remediation**: Explicit permit filters for loopback (`127.0.0.1`, `::1`) and DHCP ports.

### AUDIT-08: PID Reuse Vulnerability
- **File**: `src-tauri/src/process/manager.rs:136-162`
- **Finding**: Process tracking keyed only on PID without creation timestamps.
- **Impact**: Recycled PIDs could be misattributed.
- **Remediation**: Track process start time using `GetProcessTimes` alongside PID and normalized image path.

### AUDIT-09: Unimplemented Rate Limiting
- **File**: `src-tauri/src/commands/mod.rs:80-87`
- **Finding**: Download and upload limits saved in SQLite were not enforced in the networking engine.
- **Impact**: UI indicated rate limiting, but traffic was either blocked or unrestricted.
- **Remediation**: Disclosed technical boundaries; clearly marked profiles as unenforced in user-mode ALE.

---

## BandwidthX 2.1 — Production Enforcement Hardening

### AUDIT-10: Hardcoded Target PID & Inadequate Process Validation in Service
- **File**: `windows-service/src/main.rs:94-106`
- **Finding**: Service hardcoded `target_pid: 1` and validated path existence on disk rather than checking the running process object.
- **Remediation**: Passed PID and creation time over IPC; validated running process via `OpenProcess`, `QueryFullProcessImageNameW`, and `GetProcessTimes`.

### AUDIT-11: Target Application's Existing TCP Sockets Teardown Defect
- **File**: `crates/common/src/wfp.rs:248-285`
- **Finding**: Target application existing sockets were reset during connection teardown.
- **Remediation**: Added explicit exclusion for `permitted_pid`, guaranteeing target connections survive Exclusive Mode activation.

### AUDIT-12: Unrestricted Named Pipe Access and Missing Client Authentication
- **File**: `windows-service/src/main.rs:215-265`
- **Finding**: Named Pipe lacked DACL and client elevation checks.
- **Remediation**: Enforced SDDL DACL `D:(A;;GA;;;BA)(A;;GA;;;SY)` and verified client token elevation.

### AUDIT-13: Silent Fallback to Local Engine Masked Inactive Service
- **File**: `src-tauri/src/service/client.rs:36-120`
- **Finding**: Silent fallback masked whether service was offline.
- **Remediation**: Removed silent fallback; service is sole enforcement authority.

### AUDIT-14: Incomplete DNS/DHCP Exemption Scope & System Processes
- **File**: `crates/common/src/wfp.rs:88-135`
- **Finding**: DHCPv6 and system services (`svchost`, `lsass`, `services`) lacked explicit ALE permit rules.
- **Remediation**: Added DHCPv6 (UDP 546/547) and Core Windows Services ALE App ID permits at weight 10.

### AUDIT-15: Dual-Stack TCP EStats Collection & Socket Cache Eviction
- **File**: `src-tauri/src/network/iphelper.rs:300-380`
- **Finding**: EStats only sampled IPv4; socket cache grew unbounded.
- **Remediation**: Implemented dual IPv4/IPv6 EStats with 5s socket eviction.

### AUDIT-16: Unenforced Rate Limiting Presented as Active Shaping
- **File**: `src/components/applications/ApplicationsPage.tsx`
- **Finding**: Rate limiting implied active kernel pacing.
- **Remediation**: Changed badges to `LIMIT PROFILE (UNENFORCED)` with explanatory notices.

### AUDIT-17: Standalone Service Control Manager (SCM) Management Missing
- **File**: `windows-service/src/main.rs:15-85`
- **Finding**: Service lacked SCM installation/control CLI flags.
- **Remediation**: Implemented native Win32 SCM CLI options (`--install`, `--uninstall`, `--start`, `--stop`, `--status`, `--version`).

---

## BandwidthX 2.2 — Windows Service Lifecycle & Enforcement Validation

### AUDIT-18: SCM Service Dispatcher and Lifecycle Missing
- **File**: `windows-service/src/main.rs:395-500`
- **Finding**: In v2.1, the service binary executed a Tokio main function but did not implement the Windows Service Control Manager dispatcher. When started by SCM, the service process failed with error 1053 ("The service did not respond to the start or control request in a timely fashion").
- **Impact**: Service could only be run in an interactive console session and failed when managed as an actual Windows background service.
- **Remediation**:
  1. Implemented `StartServiceCtrlDispatcherW` with `SERVICE_TABLE_ENTRYW`.
  2. Implemented `service_main` entrypoint with `RegisterServiceCtrlHandlerExW`.
  3. Managed accurate `SERVICE_STATUS` lifecycle transitions (`SERVICE_START_PENDING` -> `SERVICE_RUNNING` -> `SERVICE_STOP_PENDING` -> `SERVICE_STOPPED`).
  4. Implemented graceful handling of `SERVICE_CONTROL_STOP` and `SERVICE_CONTROL_SHUTDOWN`.
  5. Wrapped Tokio multi-thread runtime lifecycle inside `service_main`.
  6. Connected shutdown signal to Tokio broadcast channel and emergency WFP filter purge.
  7. Added fallback to interactive console mode when invoked from a terminal (`ERROR_FAILED_SERVICE_CONTROLLER_CONNECT` / 1063 or `--console`).

### AUDIT-19: Unsecured Fallback in Named Pipe Creation
- **File**: `windows-service/src/main.rs:250-280`
- **Finding**: In v2.1, `create_named_pipe_server` fell back to `options.create(...)` if `create_admin_security_attributes()` failed.
- **Impact**: If SDDL conversion encountered an error, the service would silently instantiate an unsecured pipe open to any local user.
- **Remediation**:
  1. Removed the unsecured fallback entirely.
  2. Implemented fail-closed semantics: startup immediately aborts if secure descriptor creation fails.
  3. Added `verify_pipe_security` querying `GetKernelObjectSecurity` with `DACL_SECURITY_INFORMATION` to guarantee the effective descriptor is attached to the pipe handle.

### AUDIT-20: Incomplete Authorization Model Relying on Elevation Alone
- **File**: `windows-service/src/main.rs:285-350`
- **Finding**: In v2.1, `authenticate_client` only inspected `TokenElevation`.
- **Impact**: Token elevation indicates high integrity, but does not confirm that the caller belongs to the Builtin Administrators group or LocalSystem principal.
- **Remediation**:
  1. Implemented client impersonation via `ImpersonateNamedPipeClient`.
  2. Checked caller group membership against the Builtin Administrators SID (`S-1-5-32-544`) using `CheckTokenMembership`.
  3. Called `RevertToSelf()` to safely restore service context.
  4. Inspected client process token for `TokenElevation`.
  5. Both criteria must pass before processing any privileged IPC command.

### AUDIT-21: Process Lifetime Pinning & Enforcement Race Conditions
- **File**: `windows-service/src/main.rs:90-180`
- **Finding**: In v2.1, `validate_running_process` closed the process handle before WFP policy installation began, and allowed `claimed_creation_time == 0`.
- **Impact**: A process could terminate and its PID could be recycled between validation and policy commitment, resulting in a race condition.
- **Remediation**:
  1. Required `claimed_creation_time > 0` (nonzero timestamp).
  2. Implemented `ValidatedProcess` which holds the open kernel `HANDLE` with `PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE`. Holding this handle prevents the kernel from destroying the `EPROCESS` block and recycling the PID.
  3. Implemented `revalidate_and_enforce` calling `WaitForSingleObject(handle, 0)` immediately before WFP commitment. If the target process terminates during policy preparation, enforcement is safely aborted.

### AUDIT-22: Missing Logging and Surfacing of Failed Sockets Teardown
- **File**: `crates/common/src/wfp.rs:420-490`
- **Finding**: In v2.1, failed `SetTcpEntry` calls were ignored, and IPv6 limitations were not surfaced in the state model.
- **Impact**: Sockets that failed to reset were not accounted for in analytics or UI status.
- **Remediation**:
  1. Implemented `TeardownReport` struct surfacing `ipv4_terminated`, `ipv4_failed`, `ipv6_unmanaged`, and `ipv6_notice`.
  2. Logged all `SetTcpEntry` failures with error codes.
  3. Explicitly reported that user-mode IP Helper lacks `SetTcp6Entry`, with ALE blocking subsequent packet transactions.
  4. Attached `teardown_report` to `ExclusiveModeState`.

### AUDIT-23: Service Recovery Configuration Missing in SCM Registration
- **File**: `windows-service/src/main.rs:620-650`
- **Finding**: `install_service` did not configure service failure restart actions in SCM.
- **Impact**: If the service unexpectedly failed or was terminated, Windows SCM did not restart it.
- **Remediation**:
  1. Configured `ChangeServiceConfig2W` with `SERVICE_CONFIG_FAILURE_ACTIONS` and `SERVICE_FAILURE_ACTIONSW`.
  2. Configured automatic restart actions: 1st failure (5s delay), 2nd failure (10s delay), reset period (86,400s / 24 hours).

### AUDIT-24: Dynamic Named Pipe DACL Parsing and Explicit ACE Validation
- **File**: `windows-service/src/main.rs:255-345`
- **Finding**: In v2.2, pipe security relied on `ConvertStringSecurityDescriptorToSecurityDescriptorW` without inspecting the effective DACL applied to the kernel object handle by `CreateNamedPipeW`.
- **Impact**: If the security descriptor string was altered, bypassed, or inherited unwanted ACEs, unauthorized non-admin processes could potentially open the named pipe.
- **Remediation**:
  1. Implemented `verify_pipe_security(pipe_handle)`.
  2. Queried effective DACL directly from the kernel object using `GetKernelObjectSecurity(DACL_SECURITY_INFORMATION)`.
  3. Extracted DACL with `GetSecurityDescriptorDacl` and iterated every ACE entry using `GetAce`.
  4. Enforced that only `ACCESS_ALLOWED_ACE_TYPE` entries exist.
  5. Validated exact SID matches against Builtin Administrators (`S-1-5-32-544`) and LocalSystem (`S-1-5-18`) using `EqualSid`.
  6. Rejected any unexpected or excessively permissive ACEs with immediate server startup abort.

### AUDIT-25: Impersonation Ordering & Guaranteed RevertToSelf via RAII Guard
- **File**: `windows-service/src/main.rs:456-545`
- **Finding**: In v2.2, client impersonation occurred before bounded frame reading, and error returns could theoretically leak impersonated tokens.
- **Impact**: Impersonation before payload framing allowed unauthenticated clients to hold pipe sessions without sending data. If an error occurred during privilege checking, missing `RevertToSelf` would leave the worker thread executing under the client's token.
- **Remediation**:
  1. Corrected authorization sequence: Accept connection -> read 4-byte bounded length header (rejecting frames > 1MB) -> read full payload -> decode frame -> impersonate client -> validate BUILTIN\Administrators via `CheckTokenMembership` -> revert impersonation -> validate process token `TokenElevation` -> only then execute privileged request.
  2. Implemented `ImpersonationGuard`: an RAII guard that calls `ImpersonateNamedPipeClient` on creation and guarantees `RevertToSelf()` runs on drop across all success and error unwind paths.

### AUDIT-26: Non-Blocking SCM Control Handler & Asynchronous Emergency Restoration
- **File**: `windows-service/src/main.rs:670-750`
- **Finding**: In v2.2, `service_control_handler` called `engine.emergency_restore()` directly inside the SCM callback thread.
- **Impact**: SCM control handlers must respond within 30 seconds (preferably milliseconds). If WFP transaction abort or filter enumeration blocked or deadlocked inside the SCM callback, Windows SCM reported Error 1053 ("The service did not respond to the start or control request in a timely fashion").
- **Remediation**:
  1. Replaced blocking calls in `service_control_handler` with an instantaneous non-blocking broadcast signal to Tokio runtime via `SHUTDOWN_TX`.
  2. Handler immediately sets status to `SERVICE_STOP_PENDING` with `dwWaitHint = 10000` and returns `0` (`NO_ERROR`) to SCM.
  3. Bounded emergency restoration (5s timeout via `tokio::time::timeout`) executes inside the Tokio service execution context.
  4. `SERVICE_STOPPED` status is reported to SCM strictly after WFP cleanup completes.

### AUDIT-27: Authoritative SCM Failure Action Verification
- **File**: `windows-service/src/main.rs:845-920`
- **Finding**: In v2.2, `ChangeServiceConfig2W` results were not verified against the active SCM configuration, risking silent failure during service setup.
- **Impact**: If failure actions failed to apply, the service would report successful installation but fail to recover from crashes.
- **Remediation**:
  1. Checked every `ChangeServiceConfig2W` result; if failure actions configuration fails, installation aborts and automatically deletes the created service.
  2. Queried active configuration via `QueryServiceConfig2W(SERVICE_CONFIG_FAILURE_ACTIONS)`.
  3. Verified that exactly 3 failure actions (`cActions == 3`) with expected delay parameters (5000ms, 10000ms, 0) are actively registered in the Windows SCM database.

### AUDIT-28: Genuine Live Windows SCM Integration Tests Replacing Placeholders
- **File**: `windows-service/tests/real_windows_scm_integration.rs`
- **Finding**: Prior tests included placeholder assertions and mock checks rather than real SCM and WFP kernel operations.
- **Impact**: No automated end-to-end assurance that the service binary actually registered with Windows SCM, filtered kernel network packets, survived target connections, and recovered properly.
- **Remediation**:
  1. Replaced all placeholder tests with 20 genuine Windows integration tests:
     - Real SCM service installation, startup, and state polling (`SERVICE_RUNNING`).
     - Real named pipe IPC framing and Ping/Pong verification.
     - Security rejection of oversized/malformed frames (>1MB) with HTTP 400.
     - Spawning real Windows target child process (`powershell.exe`) and querying actual PID and FILETIME creation timestamp.
     - Live Exclusive Mode activation via IPC with WFP filter injection.
     - Live TCP echo socket verification proving target application network access.
     - Verification of non-target network blocking under Exclusive Mode.
     - Existing target TCP socket survival test proving established connections are preserved.
     - IPv4 TCP teardown behavior validation via live TCP table enumeration.
     - Explicit disclosure asserting user-mode IP Helper limitation on IPv6 teardown.
     - IPC Exclusive Mode deactivation and return of normal connectivity.
     - SCM graceful service stop and verification of `SERVICE_STOPPED`.
     - SCM service restart and verification of configured failure recovery actions.
     - Real emergency restoration with active WFP filters and verification of 0 remaining filters.
  2. Implemented `scripts/run_scm_integration_test.ps1` with strict administrator privilege assertion, release compilation, live SCM orchestration, and fail-safe `try/finally` cleanup.
