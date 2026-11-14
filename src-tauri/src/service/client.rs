use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;
use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus, ExclusiveModeState
};

/// Authoritative client bridge communicating with the standalone BandwidthX Privileged Service.
/// Silent local engine fallback is explicitly removed to ensure a single kernel enforcement authority.
pub struct ServiceClient {
    // Fail-safe engine used exclusively for emergency local purge if service is halted
    failsafe_engine: HardenedWfpEngine,
}

impl ServiceClient {
    pub fn new() -> Self {
        Self {
            failsafe_engine: HardenedWfpEngine::new(),
        }
    }

    /// Sends an IPC request to the privileged Windows Service via length-prefixed Named Pipe
    pub async fn send_ipc_request(&self, request: &IpcRequest) -> Result<IpcResponse, String> {
        let mut client = match ClientOptions::new().open(BANDWIDTHX_IPC_PIPE_NAME) {
            Ok(pipe) => pipe,
            Err(e) => return Err(format!("BandwidthX Service is offline or unreachable: {}", e)),
        };

        let frame = encode_frame(request).map_err(|e| format!("IPC encoding error: {}", e))?;
        client.write_all(&frame).await.map_err(|e| format!("IPC write error: {}", e))?;

        let mut len_buf = [0u8; 4];
        client.read_exact(&mut len_buf).await.map_err(|e| format!("IPC read length error: {}", e))?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > 1024 * 1024 {
            return Err("IPC response frame exceeded maximum allowed bounds (1MB)".to_string());
        }

        let mut payload = vec![0u8; len];
        client.read_exact(&mut payload).await.map_err(|e| format!("IPC read payload error: {}", e))?;

        let mut full_frame = Vec::with_capacity(4 + len);
        full_frame.extend_from_slice(&len_buf);
        full_frame.extend_from_slice(&payload);

        decode_frame::<IpcResponse>(&full_frame).map_err(|e| format!("IPC decode error: {}", e))
    }

    /// Queries authoritative service status directly from the running service
    pub fn get_service_status(&self) -> ServiceStatus {
        let is_elevated = HardenedWfpEngine::is_admin();

        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let res = tokio::task::block_in_place(|| {
                handle.block_on(async {
                    tokio::time::timeout(Duration::from_millis(500), self.send_ipc_request(&IpcRequest::GetStatus)).await
                })
            });

            if let Ok(Ok(IpcResponse::Status(status))) = res {
                return status;
            }
        }

        // Service is offline: return accurate disconnected state without simulation
        ServiceStatus {
            is_running: false,
            is_elevated,
            is_service_connected: false,
            exclusive_mode_active: false,
            active_filter_count: 0,
            version: "2.1.0".to_string(),
        }
    }

    /// Queries authoritative Exclusive Mode state directly from the service
    pub fn get_exclusive_mode_state(&self) -> ExclusiveModeState {
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let res = tokio::task::block_in_place(|| {
                handle.block_on(async {
                    tokio::time::timeout(Duration::from_millis(500), self.send_ipc_request(&IpcRequest::GetExclusiveState)).await
                })
            });

            if let Ok(Ok(IpcResponse::ExclusiveState(state))) = res {
                return state;
            }
        }

        ExclusiveModeState::default()
    }

    /// Commands the privileged service to enable Exclusive Mode.
    /// Rejects with an error if the service is offline (no silent fallback).
    pub fn enable_exclusive_mode(
        &self,
        target_pid: u32,
        target_creation_time: u64,
        exe_path: &str,
        app_name: &str,
    ) -> Result<ExclusiveModeState, String> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| format!("Tokio runtime error: {}", e))?;

        let req = IpcRequest::EnableExclusiveMode {
            target_pid,
            target_creation_time,
            target_exe_path: exe_path.to_string(),
            target_app_name: app_name.to_string(),
            exemptions: vec![],
        };

        let response = tokio::task::block_in_place(|| {
            rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(3), self.send_ipc_request(&req)).await
            })
        })
        .map_err(|_| "Timeout waiting for BandwidthX Service to configure WFP policies".to_string())?
        .map_err(|e| format!("Failed to reach BandwidthX Service: {}. Please ensure BandwidthX Service is started.", e))?;

        match response {
            IpcResponse::Ok { .. } => Ok(self.get_exclusive_mode_state()),
            IpcResponse::Error { message, .. } => Err(message),
            _ => Err("Unexpected response from BandwidthX Service".to_string()),
        }
    }

    /// Commands the privileged service to disable Exclusive Mode.
    pub fn disable_exclusive_mode(&self) -> Result<ExclusiveModeState, String> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| format!("Tokio runtime error: {}", e))?;

        let response = tokio::task::block_in_place(|| {
            rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(3), self.send_ipc_request(&IpcRequest::DisableExclusiveMode)).await
            })
        })
        .map_err(|_| "Timeout waiting for service to disable Exclusive Mode".to_string())?
        .map_err(|e| format!("Failed to communicate with service: {}", e))?;

        match response {
            IpcResponse::Ok { .. } => Ok(self.get_exclusive_mode_state()),
            IpcResponse::Error { message, .. } => Err(message),
            _ => Err("Unexpected response from BandwidthX Service".to_string()),
        }
    }

    /// Commands the privileged service to block an application executable.
    pub fn block_application(&self, exe_path: &str, app_name: &str) -> Result<(), String> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| format!("Tokio runtime error: {}", e))?;

        let rule = bandwidthx_common::NetworkRule {
            id: format!("block-{}", app_name),
            app_name: app_name.to_string(),
            executable_path: exe_path.to_string(),
            rule_type: bandwidthx_common::RuleType::Block,
            direction: bandwidthx_common::Direction::Both,
            limit_bytes_per_sec: None,
            enabled: true,
            created_at: chrono::Local::now().to_rfc3339(),
            updated_at: chrono::Local::now().to_rfc3339(),
        };

        let response = tokio::task::block_in_place(|| {
            rt.block_on(async {
                tokio::time::timeout(Duration::from_secs(3), self.send_ipc_request(&IpcRequest::ApplyRule { rule })).await
            })
        })
        .map_err(|_| "Timeout waiting for service to apply rule".to_string())?
        .map_err(|e| format!("Failed to communicate with service: {}", e))?;

        match response {
            IpcResponse::Ok { .. } => Ok(()),
            IpcResponse::Error { message, .. } => Err(message),
            _ => Err("Unexpected response from BandwidthX Service".to_string()),
        }
    }

    /// Triggers emergency restore via service if connected, and purges local engine as a safety net.
    pub fn emergency_restore(&self) {
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let _ = tokio::task::block_in_place(|| {
                handle.block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), self.send_ipc_request(&IpcRequest::EmergencyRestore)).await
                })
            });
        }
        self.failsafe_engine.emergency_restore();
    }
}
