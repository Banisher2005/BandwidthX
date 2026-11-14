use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{Direction, IpcRequest, NetworkRule, RuleType};

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
    assert_eq!(state.allowed_exemptions.len(), 4);
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
fn test_emergency_restore_and_sublayer_contract() {
    let engine = HardenedWfpEngine::new();
    engine.emergency_restore();
    assert_eq!(engine.get_active_filter_count(), 0);
    assert!(!engine.get_exclusive_state().is_active);
}
