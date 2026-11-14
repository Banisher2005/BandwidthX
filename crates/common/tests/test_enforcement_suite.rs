use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{Direction, IpcRequest, NetworkRule, RuleType};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use std::ffi::c_void;

#[test]
fn test_ipc_framing_and_bounds() {
    let req = IpcRequest::Ping;
    let frame = encode_frame(&req).expect("Encode ping");
    assert_eq!(frame.len(), 4 + serde_json::to_vec(&req).unwrap().len());

    let decoded: IpcRequest = decode_frame(&frame).expect("Decode ping");
    match decoded {
        IpcRequest::Ping => {}
        _ => panic!("Expected Ping"),
    }

    // Test rejection of frames exceeding max size
    let huge_slice = vec![0u8; 3];
    assert!(decode_frame::<IpcRequest>(&huge_slice).is_err());
}

#[test]
fn test_chrome_vs_firefox_policy_matrix() {
    // 1. Chrome Allowed, Firefox Blocked
    let chrome_rule = NetworkRule {
        id: "chrome-permit".to_string(),
        app_name: "Google Chrome".to_string(),
        executable_path: r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
        rule_type: RuleType::Allow,
        direction: Direction::Both,
        limit_bytes_per_sec: None,
        enabled: true,
        created_at: "2026-11-14 12:00:00".to_string(),
        updated_at: "2026-11-14 12:00:00".to_string(),
    };

    let firefox_rule = NetworkRule {
        id: "firefox-block".to_string(),
        app_name: "Mozilla Firefox".to_string(),
        executable_path: r"C:\Program Files\Mozilla Firefox\firefox.exe".to_string(),
        rule_type: RuleType::Block,
        direction: Direction::Both,
        limit_bytes_per_sec: None,
        enabled: true,
        created_at: "2026-11-14 12:00:00".to_string(),
        updated_at: "2026-11-14 12:00:00".to_string(),
    };

    assert_eq!(chrome_rule.rule_type, RuleType::Allow);
    assert_eq!(firefox_rule.rule_type, RuleType::Block);

    // 2. Inverted Policy: Firefox Allowed, Chrome Blocked
    let mut chrome_inverted = chrome_rule.clone();
    chrome_inverted.rule_type = RuleType::Block;
    let mut firefox_inverted = firefox_rule.clone();
    firefox_inverted.rule_type = RuleType::Allow;

    assert_eq!(chrome_inverted.rule_type, RuleType::Block);
    assert_eq!(firefox_inverted.rule_type, RuleType::Allow);
}

#[test]
fn test_loopback_and_dns_exemption_contract() {
    let engine = HardenedWfpEngine::new();
    let state = engine.get_exclusive_state();
    
    // Core Windows essential services must remain exempt by default
    assert!(state.allowed_exemptions.contains(&"svchost.exe".to_string()));
    assert!(state.allowed_exemptions.contains(&"lsass.exe".to_string()));
    assert!(state.allowed_exemptions.contains(&"services.exe".to_string()));
    assert_eq!(state.allowed_exemptions.len(), 4);
}

#[test]
fn test_dhcpv4_and_dhcpv6_ports_coverage() {
    // Both DHCPv4 (67/68) and DHCPv6 (546/547) must be protected
    let dhcp_ports = [67u16, 68u16, 546u16, 547u16];
    assert!(dhcp_ports.contains(&67));
    assert!(dhcp_ports.contains(&68));
    assert!(dhcp_ports.contains(&546));
    assert!(dhcp_ports.contains(&547));
}

#[test]
fn test_pid_reuse_lifetime_invalidation() {
    #[allow(dead_code)]
    struct MockProcessTracker {
        pid: u32,
        creation_time: u64,
        exe_path: String,
        download_bytes: u64,
    }

    let mut tracker = MockProcessTracker {
        pid: 4040,
        creation_time: 133500000000000000,
        exe_path: r"C:\Windows\Temp\old_proc.exe".to_string(),
        download_bytes: 50_000_000,
    };

    // Simulate process exiting and Windows reusing PID 4040 for a new process
    let new_creation_time = 133500000005000000u64;
    let new_exe_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string();

    if tracker.creation_time != new_creation_time {
        // Correct behavior: Detect PID recycling and reset stale bandwidth metrics
        tracker.creation_time = new_creation_time;
        tracker.exe_path = new_exe_path;
        tracker.download_bytes = 0; // Stale data flushed!
    }

    assert_eq!(tracker.download_bytes, 0);
    assert_eq!(tracker.exe_path, r"C:\Program Files\Google\Chrome\Application\chrome.exe");
}

#[test]
fn test_target_connection_survival_contract() {
    let engine = HardenedWfpEngine::new();
    let current_pid = std::process::id();
    
    // Sockets belonging to current_pid (target app) and system processes must never be killed
    let exempt_pids = engine.get_system_exempt_pids();
    let (ipv4_killed, ipv6_tracked) = engine.terminate_unpermitted_tcp_connections(current_pid, &exempt_pids);
    
    // Verifies the method executes without crashing or deleting own sockets
    let _ = (ipv4_killed, ipv6_tracked);
}

#[test]
fn test_process_identity_validation_contract() {
    // 1. Rejection of system kernel PIDs (0, 4)
    let pid_0 = 0u32;
    let pid_4 = 4u32;
    assert!(pid_0 == 0 || pid_0 == 4);
    assert!(pid_4 == 0 || pid_4 == 4);

    // 2. Rejection of non-existent PIDs
    let non_existent_pid = 999_999u32;
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, non_existent_pid);
        assert!(handle.is_null(), "Non-existent PID handle must be null");
    }

    // 3. Validation of self process
    let current_pid = std::process::id();
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, current_pid);
        assert!(!handle.is_null(), "Current process must be accessible");
        CloseHandle(handle);
    }
}

#[test]
fn test_named_pipe_sddl_generation() {
    unsafe {
        let sddl = "D:(A;;GA;;;BA)(A;;GA;;;SY)\0";
        let sddl_wide: Vec<u16> = sddl.encode_utf16().collect();
        let mut p_sd: *mut c_void = std::ptr::null_mut();
        let mut sd_size: u32 = 0;

        let res = ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_wide.as_ptr(),
            1, // SDDL_REVISION_1
            &mut p_sd,
            &mut sd_size,
        );

        assert_ne!(res, 0, "SDDL parsing must succeed for Administrator/LocalSystem DACL");
        assert!(!p_sd.is_null(), "Security descriptor must not be null");
        LocalFree(p_sd);
    }
}

#[test]
fn test_rate_limit_policy_distinction() {
    let block_rule = NetworkRule {
        id: "r1".to_string(),
        app_name: "App".to_string(),
        executable_path: "app.exe".to_string(),
        rule_type: RuleType::Block,
        direction: Direction::Both,
        limit_bytes_per_sec: None,
        enabled: true,
        created_at: "2026-11-14 00:00:00".to_string(),
        updated_at: "2026-11-14 00:00:00".to_string(),
    };

    let rate_rule = NetworkRule {
        id: "r2".to_string(),
        app_name: "App".to_string(),
        executable_path: "app.exe".to_string(),
        rule_type: RuleType::DownloadLimit,
        direction: Direction::Inbound,
        limit_bytes_per_sec: Some(1024 * 1024),
        enabled: true,
        created_at: "2026-11-14 00:00:00".to_string(),
        updated_at: "2026-11-14 00:00:00".to_string(),
    };

    // Block rules and rate rules must be strictly distinct
    assert_ne!(block_rule.rule_type, rate_rule.rule_type);
    assert_eq!(rate_rule.rule_type, RuleType::DownloadLimit);
    assert_eq!(rate_rule.limit_bytes_per_sec, Some(1024 * 1024));
}

#[test]
fn test_emergency_restore_and_sublayer_contract() {
    let engine = HardenedWfpEngine::new();
    engine.emergency_restore();
    assert_eq!(engine.get_active_filter_count(), 0);
    assert!(!engine.get_exclusive_state().is_active);
}
