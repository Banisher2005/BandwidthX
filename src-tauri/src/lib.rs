pub mod network;
pub mod process;
pub mod storage;
pub mod service;
pub mod commands;

use tauri::Manager;
use commands::AppState;
use network::NetworkTelemetryEngine;
use process::ProcessManager;
use service::ServiceClient;
use storage::Database;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("bandwidthx.db");

    let db = Database::new(db_path).expect("Failed to initialize SQLite database");
    let state = AppState {
        telemetry: NetworkTelemetryEngine::new(),
        process_mgr: ProcessManager::new(),
        service: ServiceClient::new(),
        db,
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_global_network_stats,
            commands::get_process_network_list,
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
                    state.service.local_engine.emergency_restore();
                }
            }
        });
}
