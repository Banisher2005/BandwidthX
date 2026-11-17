use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

use bandwidthx_common::{GlobalNetworkStats, ProcessNetworkInfo};
use crate::network::iphelper::NetworkTelemetryEngine;
use crate::process::ProcessManager;

#[derive(Clone)]
pub struct TelemetrySnapshot {
    pub global_stats: GlobalNetworkStats,
    pub processes: Vec<ProcessNetworkInfo>,
    pub timestamp: Instant,
}

pub struct CentralizedTelemetryManager {
    telemetry_engine: Arc<NetworkTelemetryEngine>,
    process_manager: Arc<ProcessManager>,
    cached_snapshot: RwLock<Option<TelemetrySnapshot>>,
}

impl CentralizedTelemetryManager {
    pub fn new(telemetry_engine: Arc<NetworkTelemetryEngine>, process_manager: Arc<ProcessManager>) -> Self {
        Self {
            telemetry_engine,
            process_manager,
            cached_snapshot: RwLock::new(None),
        }
    }

    /// Obtains an atomic, synchronized telemetry snapshot.
    /// If an existing snapshot was collected within the cache duration (800ms),
    /// returns the cached copy to eliminate redundant process scans and EStats wipeouts.
    pub fn get_snapshot(
        &self,
        exclusive_target_path: Option<&str>,
        blocked_paths: &[String],
    ) -> TelemetrySnapshot {
        let max_age = Duration::from_millis(800);

        // Check if cached snapshot is sufficiently fresh
        {
            let cache = self.cached_snapshot.read();
            if let Some(ref snapshot) = *cache {
                if snapshot.timestamp.elapsed() < max_age {
                    return snapshot.clone();
                }
            }
        }

        // Perform unified single-pass sampling
        self.sample_fresh(exclusive_target_path, blocked_paths)
    }

    /// Gathers live metrics from Windows IP Helper, TCP EStats, and ToolHelp32 process snapshots
    pub fn sample_fresh(
        &self,
        exclusive_target_path: Option<&str>,
        blocked_paths: &[String],
    ) -> TelemetrySnapshot {
        // 1. Poll aggregate interface throughput and EStats per-PID metrics in a single pass
        let mut global_stats = self.telemetry_engine.poll_global_stats();
        let sockets_by_pid = self.telemetry_engine.get_sockets_by_pid();
        let measured_traffic = self.telemetry_engine.get_measured_pid_traffic();

        global_stats.detected_applications = sockets_by_pid.len();

        // 2. Associate with running processes and validate PID lifetime
        let processes = self.process_manager.get_processes_with_network(
            &sockets_by_pid,
            &measured_traffic,
            exclusive_target_path,
            blocked_paths,
        );

        let snapshot = TelemetrySnapshot {
            global_stats,
            processes,
            timestamp: Instant::now(),
        };

        // Cache the updated snapshot
        *self.cached_snapshot.write() = Some(snapshot.clone());

        snapshot
    }

    pub fn get_global_stats(&self, exclusive_target: Option<&str>, blocked: &[String]) -> GlobalNetworkStats {
        self.get_snapshot(exclusive_target, blocked).global_stats
    }

    pub fn get_processes(&self, exclusive_target: Option<&str>, blocked: &[String]) -> Vec<ProcessNetworkInfo> {
        self.get_snapshot(exclusive_target, blocked).processes
    }
}
