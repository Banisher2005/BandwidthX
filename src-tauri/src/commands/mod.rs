use tauri::State;
use bandwidthx_common::{
    GlobalNetworkStats, ProcessNetworkInfo, ExclusiveModeState, ServiceStatus, NetworkRule
};
use crate::network::NetworkTelemetryEngine;
use crate::process::ProcessManager;
use crate::service::ServiceClient;
use crate::storage::Database;

pub struct AppState {
    pub telemetry: NetworkTelemetryEngine,
    pub process_mgr: ProcessManager,
    pub service: ServiceClient,
    pub db: Database,
}

#[tauri::command]
pub fn get_global_network_stats(state: State<'_, AppState>) -> GlobalNetworkStats {
    let mut stats = state.telemetry.poll_global_stats();
    let sockets = state.telemetry.get_sockets_by_pid();
    stats.detected_applications = sockets.len();
    stats
}

#[tauri::command]
pub fn get_process_network_list(state: State<'_, AppState>) -> Vec<ProcessNetworkInfo> {
    let stats = state.telemetry.poll_global_stats();
    let sockets = state.telemetry.get_sockets_by_pid();
    let exclusive_state = state.service.local_engine.get_exclusive_state();
    
    // Get list of blocked paths from rules
    let rules = state.db.get_rules().unwrap_or_default();
    let blocked_paths: Vec<String> = rules.iter()
        .filter(|r| r.enabled && r.rule_type == bandwidthx_common::RuleType::Block)
        .map(|r| r.executable_path.clone())
        .collect();

    let procs = state.process_mgr.get_processes_with_network(
        &sockets,
        stats.download_speed_bps,
        stats.upload_speed_bps,
        exclusive_state.target_exe_path.as_deref(),
        &blocked_paths,
    );

    // Periodically log top consumers into analytics database
    for p in procs.iter().take(3) {
        if p.download_bytes_sec > 0 || p.upload_bytes_sec > 0 {
            let _ = state.db.log_analytics(&p.name, &p.exe_path, p.download_bytes_sec, p.upload_bytes_sec);
        }
    }

    procs
}

#[tauri::command]
pub fn get_exclusive_mode_state(state: State<'_, AppState>) -> ExclusiveModeState {
    state.service.local_engine.get_exclusive_state()
}

#[tauri::command]
pub fn enable_exclusive_mode(
    state: State<'_, AppState>,
    target_pid: u32,
    exe_path: String,
    app_name: String,
) -> Result<ExclusiveModeState, String> {
    state.service.local_engine.enable_exclusive_mode(target_pid, &exe_path, &app_name)
}

#[tauri::command]
pub fn disable_exclusive_mode(state: State<'_, AppState>) -> ExclusiveModeState {
    state.service.local_engine.disable_exclusive_mode()
}

#[tauri::command]
pub fn get_rules(state: State<'_, AppState>) -> Vec<NetworkRule> {
    state.db.get_rules().unwrap_or_default()
}

#[tauri::command]
pub fn save_rule(state: State<'_, AppState>, rule: NetworkRule) -> Result<NetworkRule, String> {
    if rule.enabled && rule.rule_type == bandwidthx_common::RuleType::Block {
        let _ = state.service.local_engine.block_application(&rule.executable_path, &rule.app_name);
    }
    state.db.save_rule(&rule).map_err(|e| e.to_string())?;
    Ok(rule)
}

#[tauri::command]
pub fn delete_rule(state: State<'_, AppState>, rule_id: String, _exe_path: String) -> Result<bool, String> {
    state.db.delete_rule(&rule_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn emergency_restore(state: State<'_, AppState>) -> bool {
    state.service.local_engine.emergency_restore();
    true
}

#[tauri::command]
pub fn get_service_status(state: State<'_, AppState>) -> ServiceStatus {
    state.service.get_service_status()
}

#[tauri::command]
pub fn get_analytics(state: State<'_, AppState>, range: String) -> Result<Vec<serde_json::Value>, String> {
    state.db.get_analytics(&range).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_analytics_csv(state: State<'_, AppState>, _range: String) -> Result<String, String> {
    state.db.export_csv().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(_state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({
        "start_with_windows": false,
        "start_minimized": false,
        "monitoring_interval_ms": 1000,
        "data_retention_days": 30,
        "selected_adapter": "Auto-detect active",
        "notifications_enabled": true,
        "theme": "charcoal-dark",
        "fail_safe_timeout_seconds": 30,
    })
}

#[tauri::command]
pub fn save_settings(_state: State<'_, AppState>, _settings: serde_json::Value) -> bool {
    true
}
