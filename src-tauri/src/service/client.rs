use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;
use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus, ExclusiveModeState
};

pub struct ServiceClient {
    // In-process fallback engine if service is not running and user is elevated
    pub local_engine: HardenedWfpEngine,
}

impl ServiceClient {
    pub fn new() -> Self {
        Self {
            local_engine: HardenedWfpEngine::new(),
        }
    }

    /// Attempts to send an IPC request to the privileged Windows Service via Named Pipe
    pub async fn send_ipc_request(&self, request: &IpcRequest) -> Result<IpcResponse, String> {
        let mut client = match ClientOptions::new().open(BANDWIDTHX_IPC_PIPE_NAME) {
            Ok(pipe) => pipe,
            Err(_) => return Err("Service named pipe unavailable".to_string()),
        };

        let frame = encode_frame(request).map_err(|e| e.to_string())?;
        client.write_all(&frame).await.map_err(|e| e.to_string())?;

        let mut len_buf = [0u8; 4];
        client.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut payload = vec![0u8; len];
        client.read_exact(&mut payload).await.map_err(|e| e.to_string())?;

        let mut full_frame = Vec::with_capacity(4 + len);
        full_frame.extend_from_slice(&len_buf);
        full_frame.extend_from_slice(&payload);

        decode_frame::<IpcResponse>(&full_frame).map_err(|e| e.to_string())
    }

    pub fn get_service_status(&self) -> ServiceStatus {
        // Probe service status synchronously or fallback to local engine
        let is_elevated = HardenedWfpEngine::is_admin();
        let exclusive_active = self.local_engine.get_exclusive_state().is_active;
        let count = self.local_engine.get_active_filter_count();

        // Check if named pipe exists on Windows
        let service_online = std::path::Path::new(BANDWIDTHX_IPC_PIPE_NAME).exists();

        ServiceStatus {
            is_running: true,
            is_elevated,
            is_service_connected: service_online,
            exclusive_mode_active: exclusive_active,
            active_filter_count: count,
            version: "2.0.0".to_string(),
        }
    }

    pub fn enable_exclusive_mode(&self, target_pid: u32, exe_path: &str, app_name: &str) -> Result<ExclusiveModeState, String> {
        // 1. Try forwarding to standalone service first if available
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let req = IpcRequest::EnableExclusiveMode {
                target_exe_path: exe_path.to_string(),
                target_app_name: app_name.to_string(),
                exemptions: vec![],
            };
            if let Ok(Ok(IpcResponse::Ok { .. })) = tokio::task::block_in_place(|| {
                handle.block_on(async {
                    tokio::time::timeout(Duration::from_millis(500), self.send_ipc_request(&req)).await
                })
            }) {
                return Ok(self.local_engine.get_exclusive_state());
            }
        }

        // 2. Authoritative local engine fallback
        self.local_engine.enable_exclusive_mode(target_pid, exe_path, app_name, &[])
    }

    pub fn disable_exclusive_mode(&self) -> ExclusiveModeState {
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let _ = tokio::task::block_in_place(|| {
                handle.block_on(async {
                    tokio::time::timeout(Duration::from_millis(500), self.send_ipc_request(&IpcRequest::DisableExclusiveMode)).await
                })
            });
        }
        self.local_engine.disable_exclusive_mode()
    }

    pub fn block_application(&self, exe_path: &str, app_name: &str) -> Result<u64, String> {
        self.local_engine.block_application(exe_path, app_name)
    }

    pub fn emergency_restore(&self) {
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let _ = tokio::task::block_in_place(|| {
                handle.block_on(async {
                    tokio::time::timeout(Duration::from_millis(500), self.send_ipc_request(&IpcRequest::EmergencyRestore)).await
                })
            });
        }
        self.local_engine.emergency_restore();
    }
}
