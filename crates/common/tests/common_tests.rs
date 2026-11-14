use bandwidthx_common::*;

#[test]
fn test_default_exclusive_state() {
    let state = ExclusiveModeState::default();
    assert!(!state.is_active);
    assert!(state.target_pid.is_none());
    assert_eq!(state.allowed_exemptions.len(), 4);
    assert!(state.allowed_exemptions.contains(&"svchost.exe".to_string()));
}

#[test]
fn test_guid_constants() {
    assert_eq!(BANDWIDTHX_PROVIDER_GUID, "87b4699e-5bdf-4cb8-9b88-14f77c862bc1");
    assert_eq!(BANDWIDTHX_SUBLAYER_GUID, "d0b27163-95d4-4a47-a7eb-29df34220aa5");
    assert_eq!(BANDWIDTHX_IPC_PIPE_NAME, r"\\.\pipe\BandwidthX_IPC");
}

#[test]
fn test_rule_serialization() {
    let rule = NetworkRule {
        id: "rule-1".to_string(),
        app_name: "Chrome".to_string(),
        executable_path: r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
        rule_type: RuleType::Block,
        direction: Direction::Both,
        limit_bytes_per_sec: None,
        enabled: true,
        created_at: "2026-11-14 00:00:00".to_string(),
        updated_at: "2026-11-14 00:00:00".to_string(),
    };

    let json = serde_json::to_string(&rule).expect("Serialize");
    let deserialized: NetworkRule = serde_json::from_str(&json).expect("Deserialize");
    assert_eq!(rule, deserialized);
}
