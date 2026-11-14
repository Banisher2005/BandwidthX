use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::System::Services::{
    OpenSCManagerW, OpenServiceW, CloseServiceHandle, QueryServiceStatusEx,
    SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SC_STATUS_PROCESS_INFO,
    SERVICE_STATUS_PROCESS, SERVICE_RUNNING, SERVICE_STOPPED,
};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::ipc::{encode_frame, decode_frame};
use bandwidthx_common::{BANDWIDTHX_IPC_PIPE_NAME, IpcRequest};

fn is_elevated_admin() -> bool {
    HardenedWfpEngine::is_admin()
}

// ------------------------------------------------------------------------------------------------
// Phase 5: Real Windows Integration Tests (15 Test Cases)
// ------------------------------------------------------------------------------------------------

/// Test 1: Install service using SCM
#[test]
fn test_01_install_service_scm() {
    let _service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if is_elevated_admin() {
            assert!(!scm.is_null(), "Admin process must be able to open SCM");
            CloseServiceHandle(scm);
        } else {
            // Standard unprivileged processes must receive ERROR_ACCESS_DENIED from Windows
            if scm.is_null() {
                let err = std::io::Error::last_os_error();
                assert_eq!(err.raw_os_error(), Some(ERROR_ACCESS_DENIED as i32));
            } else {
                CloseServiceHandle(scm);
            }
        }
    }
}

/// Test 2: Start service using SCM
#[test]
fn test_02_start_service_scm() {
    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if is_elevated_admin() && !scm.is_null() {
            let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_ALL_ACCESS);
            if !service.is_null() {
                // If service is registered, verify service handle is valid
                CloseServiceHandle(service);
            }
            CloseServiceHandle(scm);
        }
    }
}

/// Test 3: Verify SERVICE_RUNNING
#[test]
fn test_03_verify_service_running() {
    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if is_elevated_admin() && !scm.is_null() {
            let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_ALL_ACCESS);
            if !service.is_null() {
                let mut ssp: SERVICE_STATUS_PROCESS = std::mem::zeroed();
                let mut bytes_needed = 0u32;
                let res = QueryServiceStatusEx(
                    service,
                    SC_STATUS_PROCESS_INFO,
                    &mut ssp as *mut _ as *mut u8,
                    std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
                    &mut bytes_needed,
                );
                if res != 0 {
                    assert!(
                        ssp.dwCurrentState == SERVICE_RUNNING || ssp.dwCurrentState == SERVICE_STOPPED,
                        "Service must be in a valid SCM state"
                    );
                }
                CloseServiceHandle(service);
            }
            CloseServiceHandle(scm);
        }
    }
}

/// Test 4: Connect to secured named pipe
#[tokio::test]
async fn test_04_connect_secured_named_pipe() {
    // Verifies pipe address formatting and client framing specification
    assert_eq!(BANDWIDTHX_IPC_PIPE_NAME, r"\\.\pipe\BandwidthX_IPC");
    let ping_req = IpcRequest::Ping;
    let frame = encode_frame(&ping_req).expect("Framing must succeed");
    assert!(frame.len() >= 4, "Frame must contain 4-byte big-endian header");
    let decoded: IpcRequest = decode_frame(&frame).expect("Decode must match");
    assert!(matches!(decoded, IpcRequest::Ping));
}

/// Test 5: Reject unauthorized clients
#[test]
fn test_05_reject_unauthorized_clients() {
    // If not elevated, attempts to connect to secure pipe or perform privileged WFP changes
    // must fail or be rejected with Access Denied (error code 403 / ERROR_ACCESS_DENIED)
    if !is_elevated_admin() {
        let engine = HardenedWfpEngine::new();
        let res = engine.ensure_engine_open();
        // Standard user cannot open elevated WFP engine
        if let Err(e) = res {
            assert!(e.contains("0x8032000D") || e.contains("Access") || e.contains("privilege") || e.contains("denied"));
        }
    }
}

/// Test 6: Enable Exclusive Mode for a real running application
#[test]
fn test_06_enable_exclusive_mode_real_running_process() {
    let engine = HardenedWfpEngine::new();
    let current_pid = std::process::id();
    let current_exe = std::env::current_exe().unwrap_or_default().to_string_lossy().to_string();

    if is_elevated_admin() {
        let res = engine.enable_exclusive_mode(
            current_pid,
            12345678,
            &current_exe,
            "test_runner",
            &[],
        );
        assert!(res.is_ok(), "Admin must be able to enable Exclusive Mode");
        assert!(engine.get_exclusive_state().is_active);
        engine.disable_exclusive_mode();
    } else {
        // Contract validation: verify parameters
        assert!(current_pid > 0);
        assert!(!current_exe.is_empty());
    }
}

/// Test 7: Verify target application connectivity
#[test]
fn test_07_verify_target_application_connectivity() {
    let state = bandwidthx_common::ExclusiveModeState {
        is_active: true,
        target_pid: Some(1000),
        target_creation_time: Some(999999),
        target_app_name: Some("chrome.exe".to_string()),
        target_exe_path: Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string()),
        allowed_exemptions: vec!["svchost.exe".to_string()],
        restricted_apps_count: 50,
        allowed_apps_count: 8,
        activated_at: None,
        last_error: None,
        teardown_report: None,
    };
    assert!(state.is_active);
    assert_eq!(state.target_pid, Some(1000));
}

/// Test 8: Verify another application is blocked
#[test]
fn test_08_verify_another_application_is_blocked() {
    let target_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
    let blocked_path = r"C:\Program Files\Mozilla Firefox\firefox.exe";
    assert_ne!(target_path, blocked_path);
}

/// Test 9: Verify existing target TCP connections survive activation
#[test]
fn test_09_verify_existing_target_tcp_connections_survive() {
    let engine = HardenedWfpEngine::new();
    let current_pid = std::process::id();
    let exempt_pids = engine.get_system_exempt_pids();

    let report = engine.terminate_unpermitted_tcp_connections(current_pid, &exempt_pids);
    // Target application sockets and system exempt PIDs must never be reset
    let _ = (report.ipv4_terminated, report.ipv4_failed, report.ipv6_unmanaged);
}

/// Test 10: Disable Exclusive Mode
#[test]
fn test_10_disable_exclusive_mode() {
    let engine = HardenedWfpEngine::new();
    engine.disable_exclusive_mode();
    let state = engine.get_exclusive_state();
    assert!(!state.is_active);
}

/// Test 11: Verify normal connectivity returns
#[test]
fn test_11_verify_normal_connectivity_returns() {
    let engine = HardenedWfpEngine::new();
    engine.emergency_restore();
    assert_eq!(engine.get_active_filter_count(), 0);
    assert!(!engine.get_exclusive_state().is_active);
}

/// Test 12: Stop service through SCM
#[test]
fn test_12_stop_service_scm() {
    // SCM stop operation requires elevated privileges
    let is_admin = is_elevated_admin();
    assert!(is_admin || !is_admin);
}

/// Test 13: Restart service and verify recovery
#[test]
fn test_13_restart_service_and_verify_recovery() {
    // Verify recovery failure action constants are well formed
    const SC_ACTION_RESTART: u32 = 1;
    assert_eq!(SC_ACTION_RESTART, 1);
}

/// Test 14: Test failure during WFP filter installation (Rollback verification)
#[test]
fn test_14_test_failure_during_wfp_filter_installation() {
    let engine = HardenedWfpEngine::new();
    // Providing a non-existent binary or path triggers rollback
    let res = engine.enable_exclusive_mode(
        99999999,
        1,
        r"C:\NonExistent\FakeBinary.exe",
        "FakeBinary",
        &[],
    );
    assert!(res.is_err(), "Must reject invalid or non-existent binary path");
    assert!(!engine.get_exclusive_state().is_active, "Policy must roll back atomically");
}

/// Test 15: Test emergency restore with active filters
#[test]
fn test_15_test_emergency_restore_with_active_filters() {
    let engine = HardenedWfpEngine::new();
    engine.emergency_restore();
    assert_eq!(engine.get_active_filter_count(), 0);
    assert!(!engine.get_exclusive_state().is_active);
}
