use std::sync::Arc;
use tauri::State;
use bandwidthx_common::{
    GlobalNetworkStats, ProcessNetworkInfo, ExclusiveModeState, ServiceStatus, NetworkRule
};
use crate::network::{CentralizedTelemetryManager, SpeedTestEngine, SpeedTestProgress};
use crate::process::ProcessManager;
use crate::service::{ServiceClient, WindowsServiceManager, WindowsServiceInfo};
use crate::storage::Database;

pub struct AppState {
    pub centralized_telemetry: Arc<CentralizedTelemetryManager>,
    pub process_mgr: Arc<ProcessManager>,
    pub speedtest: Arc<SpeedTestEngine>,
    pub windows_services: Arc<WindowsServiceManager>,
    pub service: ServiceClient,
    pub db: Database,
}

#[tauri::command]
pub fn get_global_network_stats(state: State<'_, AppState>) -> GlobalNetworkStats {
    let exclusive_state = state.service.get_exclusive_mode_state();
    let rules = state.db.get_rules().unwrap_or_default();
    let blocked_paths: Vec<String> = rules.iter()
        .filter(|r| r.enabled && r.rule_type == bandwidthx_common::RuleType::Block)
        .map(|r| r.executable_path.clone())
        .collect();

    state.centralized_telemetry.get_global_stats(
        exclusive_state.target_exe_path.as_deref(),
        &blocked_paths,
    )
}

#[tauri::command]
pub fn get_process_network_list(state: State<'_, AppState>) -> Vec<ProcessNetworkInfo> {
    let exclusive_state = state.service.get_exclusive_mode_state();
    
    // Get list of blocked paths from rules
    let rules = state.db.get_rules().unwrap_or_default();
    let blocked_paths: Vec<String> = rules.iter()
        .filter(|r| r.enabled && r.rule_type == bandwidthx_common::RuleType::Block)
        .map(|r| r.executable_path.clone())
        .collect();

    let procs = state.centralized_telemetry.get_processes(
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
pub fn terminate_process(
    state: State<'_, AppState>,
    pid: u32,
    claimed_creation_time: Option<u64>,
) -> Result<bool, String> {
    state.process_mgr.terminate_process(pid, claimed_creation_time.unwrap_or(0))
}

#[tauri::command]
pub async fn start_speed_test(
    state: State<'_, AppState>,
    custom_endpoint: Option<String>,
) -> Result<SpeedTestProgress, String> {
    let speedtest = Arc::clone(&state.speedtest);
    speedtest.run_test(custom_endpoint).await
}

#[tauri::command]
pub fn get_speed_test_progress(state: State<'_, AppState>) -> SpeedTestProgress {
    state.speedtest.get_progress()
}

#[tauri::command]
pub fn cancel_speed_test(state: State<'_, AppState>) -> bool {
    state.speedtest.cancel()
}

#[tauri::command]
pub fn get_windows_services_list(state: State<'_, AppState>) -> Result<Vec<WindowsServiceInfo>, String> {
    state.windows_services.list_services()
}

#[tauri::command]
pub fn start_windows_service(state: State<'_, AppState>, service_name: String) -> Result<bool, String> {
    state.windows_services.start_service(&service_name)
}

#[tauri::command]
pub fn stop_windows_service(state: State<'_, AppState>, service_name: String) -> Result<bool, String> {
    state.windows_services.stop_service(&service_name)
}

#[tauri::command]
pub fn restart_windows_service(state: State<'_, AppState>, service_name: String) -> Result<bool, String> {
    state.windows_services.restart_service(&service_name)
}

#[tauri::command]
pub fn get_exclusive_mode_state(state: State<'_, AppState>) -> ExclusiveModeState {
    state.service.get_exclusive_mode_state()
}

#[tauri::command]
pub fn enable_exclusive_mode(
    state: State<'_, AppState>,
    target_pid: u32,
    target_creation_time: Option<u64>,
    exe_path: String,
    app_name: String,
) -> Result<ExclusiveModeState, String> {
    state.service.enable_exclusive_mode(target_pid, target_creation_time.unwrap_or(0), &exe_path, &app_name)
}

#[tauri::command]
pub fn disable_exclusive_mode(state: State<'_, AppState>) -> Result<ExclusiveModeState, String> {
    state.service.disable_exclusive_mode()
}

#[tauri::command]
pub fn get_rules(state: State<'_, AppState>) -> Vec<NetworkRule> {
    state.db.get_rules().unwrap_or_default()
}

#[tauri::command]
pub fn save_rule(state: State<'_, AppState>, rule: NetworkRule) -> Result<NetworkRule, String> {
    if rule.enabled && rule.rule_type == bandwidthx_common::RuleType::Block {
        let _ = state.service.block_application(&rule.executable_path, &rule.app_name);
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
    state.service.emergency_restore();
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
        "speed_test_endpoint": "https://speed.cloudflare.com",
        "data_retention_days": 30,
        "selected_adapter": "Auto-detect active",
        "notifications_enabled": true,
        "theme": "glassmorphic-charcoal",
        "fail_safe_timeout_seconds": 30,
    })
}

#[tauri::command]
pub fn save_settings(_state: State<'_, AppState>, _settings: serde_json::Value) -> bool {
    true
}
