pub mod network;
pub mod process;
pub mod storage;
pub mod service;
pub mod commands;

use std::sync::Arc;
use tauri::Manager;
use commands::AppState;
use network::{NetworkTelemetryEngine, CentralizedTelemetryManager, SpeedTestEngine};
use process::ProcessManager;
use service::{ServiceClient, WindowsServiceManager};
use storage::Database;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("bandwidthx.db");

    let db = Database::new(db_path).expect("Failed to initialize SQLite database");
    let telemetry_engine = Arc::new(NetworkTelemetryEngine::new());
    let process_mgr = Arc::new(ProcessManager::new());
    let centralized_telemetry = Arc::new(CentralizedTelemetryManager::new(
        Arc::clone(&telemetry_engine),
        Arc::clone(&process_mgr),
    ));
    let speedtest = Arc::new(SpeedTestEngine::new());
    let windows_services = Arc::new(WindowsServiceManager::new());

    let state = AppState {
        centralized_telemetry,
        process_mgr,
        speedtest,
        windows_services,
        service: ServiceClient::new(),
        db,
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_global_network_stats,
            commands::get_process_network_list,
            commands::terminate_process,
            commands::start_speed_test,
            commands::get_speed_test_progress,
            commands::cancel_speed_test,
            commands::get_windows_services_list,
            commands::start_windows_service,
            commands::stop_windows_service,
            commands::restart_windows_service,
            commands::get_exclusive_mode_state,
            commands::enable_exclusive_mode,
            commands::disable_exclusive_mode,
            commands::get_rules,
            commands::save_rule,
            commands::delete_rule,
            commands::emergency_restore,
            commands::get_service_status,
            commands::get_analytics,
            commands::export_analytics_csv,
            commands::get_settings,
            commands::save_settings,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Ensure all dynamic WFP filters are cleaned up on exit
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state.service.emergency_restore();
                }
            }
        });
}
