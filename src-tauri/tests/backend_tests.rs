use bandwidthx_app_lib::network::NetworkTelemetryEngine;
use bandwidthx_app_lib::storage::Database;
use bandwidthx_common::{NetworkRule, RuleType, Direction};

#[test]
fn test_sqlite_rules_and_analytics() {
    let temp_db = std::env::temp_dir().join(format!("test_bandwidthx_{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let db = Database::new(temp_db.clone()).expect("Init DB");

    // Test saving rule
    let rule = NetworkRule {
        id: "test-rule-1".to_string(),
        app_name: "Spotify".to_string(),
        executable_path: r"C:\Users\Test\AppData\Roaming\Spotify\Spotify.exe".to_string(),
        rule_type: RuleType::Block,
        direction: Direction::Both,
        limit_bytes_per_sec: None,
        enabled: true,
        created_at: "2026-11-14 10:00:00".to_string(),
        updated_at: "2026-11-14 10:00:00".to_string(),
    };

    db.save_rule(&rule).expect("Save rule");
    let rules = db.get_rules().expect("Get rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].app_name, "Spotify");
    assert_eq!(rules[0].rule_type, RuleType::Block);

    // Test analytics logging
    db.log_analytics("Spotify", &rule.executable_path, 1048576, 524288).expect("Log analytics");
    let analytics = db.get_analytics("hourly").expect("Get analytics");
    assert_eq!(analytics.len(), 1);

    // Test CSV export
    let csv = db.export_csv().expect("Export CSV");
    assert!(csv.contains("Application,ExecutablePath,BytesDownloaded,BytesUploaded,Timestamp"));
    assert!(csv.contains("Spotify"));

    // Cleanup
    let _ = std::fs::remove_file(temp_db);
}

#[test]
fn test_iphelper_live_telemetry() {
    let telemetry = NetworkTelemetryEngine::new();
    let stats = telemetry.poll_global_stats();
    
    // On any active machine, bytes sent or received must be greater than zero or valid non-negative values
    assert!(stats.total_download_bytes > 0 || stats.total_upload_bytes > 0 || stats.is_connected);
    assert!(!stats.adapter_name.is_empty());

    // Test socket detection
    let (tcp, udp) = telemetry.get_active_socket_counts();
    println!("Detected {} TCP sockets and {} UDP sockets on Windows system", tcp, udp);
    assert!(tcp + udp > 0, "Should detect at least one socket on a running Windows OS");
}
