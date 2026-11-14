use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::windows::named_pipe::ServerOptions;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FwpmEngineOpen0, FwpmEngineClose0, FwpmSubLayerAdd0, FwpmFilterAdd0,
    FwpmGetAppIdFromFileName0, FwpmFreeMemory0,
    FWPM_SESSION0, FWPM_SESSION_FLAG_DYNAMIC, FWPM_SUBLAYER0, FWPM_FILTER0,
    FWPM_FILTER_CONDITION0, FWP_ACTION_PERMIT, FWP_ACTION_BLOCK,
    FWP_MATCH_EQUAL, FWP_BYTE_BLOB,
    FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_LAYER_ALE_AUTH_CONNECT_V6,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
    FWPM_CONDITION_ALE_APP_ID, FWPM_CONDITION_IP_REMOTE_PORT,
};
use windows_sys::core::GUID;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, BANDWIDTHX_SUBLAYER_GUID, IpcRequest, IpcResponse, ServiceStatus
};

fn parse_guid(s: &str) -> GUID {
    let clean = s.replace('-', "");
    let mut bytes = [0u8; 16];
    for (i, chunk) in clean.as_bytes().chunks(2).enumerate() {
        if i < 16 {
            let str_val = std::str::from_utf8(chunk).unwrap_or("00");
            bytes[i] = u8::from_str_radix(str_val, 16).unwrap_or(0);
        }
    }
    GUID {
        data1: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        data2: u16::from_be_bytes([bytes[4], bytes[5]]),
        data3: u16::from_be_bytes([bytes[6], bytes[7]]),
        data4: [bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]],
    }
}

struct ServiceWfpEngine {
    engine_handle: parking_lot::Mutex<HANDLE>,
    exclusive_active: AtomicBool,
}

unsafe impl Send for ServiceWfpEngine {}
unsafe impl Sync for ServiceWfpEngine {}

impl ServiceWfpEngine {
    fn new() -> Self {
        Self {
            engine_handle: parking_lot::Mutex::new(0 as HANDLE),
            exclusive_active: AtomicBool::new(false),
        }
    }

    fn ensure_open(&self) -> Result<HANDLE, String> {
        let mut h = self.engine_handle.lock();
        if *h != (0 as HANDLE) {
            return Ok(*h);
        }

        unsafe {
            let mut session: FWPM_SESSION0 = std::mem::zeroed();
            session.flags = FWPM_SESSION_FLAG_DYNAMIC;

            let mut engine: HANDLE = 0 as HANDLE;
            let res = FwpmEngineOpen0(std::ptr::null(), 10, std::ptr::null(), &session, &mut engine);
            if res != 0 {
                return Err(format!("Engine open error 0x{:08X}", res));
            }

            let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);
            let mut sublayer: FWPM_SUBLAYER0 = std::mem::zeroed();
            sublayer.subLayerKey = sublayer_guid;
            sublayer.weight = 0xF000;
            let mut desc_wide: Vec<u16> = "BandwidthX Privileged Sublayer".encode_utf16().chain(std::iter::once(0)).collect();
            sublayer.displayData.name = desc_wide.as_mut_ptr();
            let _ = FwpmSubLayerAdd0(engine, &sublayer, std::ptr::null_mut());

            *h = engine;
            Ok(engine)
        }
    }

    fn enable_exclusive(&self, exe_path: &str, _app_name: &str) -> Result<(), String> {
        self.emergency_restore();
        let engine = self.ensure_open()?;
        let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);

        let layers = [
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
        ];

        unsafe {
            // DNS permit
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.action.r#type = FWP_ACTION_PERMIT;
                let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                cond.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
                cond.matchType = FWP_MATCH_EQUAL;
                cond.conditionValue.r#type = 3;
                cond.conditionValue.Anonymous.uint16 = 53;
                filter.filterCondition = &mut cond;
                filter.numFilterConditions = 1;
                let mut id: u64 = 0;
                let _ = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut id);
            }

            // Target app permit
            let path_wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut app_id: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
            if FwpmGetAppIdFromFileName0(path_wide.as_ptr(), &mut app_id) == 0 && !app_id.is_null() {
                for &layer in &layers {
                    let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                    filter.layerKey = layer;
                    filter.subLayerKey = sublayer_guid;
                    filter.action.r#type = FWP_ACTION_PERMIT;
                    let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                    cond.fieldKey = FWPM_CONDITION_ALE_APP_ID;
                    cond.matchType = FWP_MATCH_EQUAL;
                    cond.conditionValue.r#type = 17;
                    cond.conditionValue.Anonymous.byteBlob = app_id;
                    filter.filterCondition = &mut cond;
                    filter.numFilterConditions = 1;
                    let mut id: u64 = 0;
                    let _ = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut id);
                }
                FwpmFreeMemory0(&mut app_id as *mut _ as *mut *mut std::ffi::c_void);
            }

            // Block all other apps
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.action.r#type = FWP_ACTION_BLOCK;
                let mut id: u64 = 0;
                let _ = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut id);
            }
        }

        self.exclusive_active.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn emergency_restore(&self) {
        let mut h = self.engine_handle.lock();
        if *h != (0 as HANDLE) {
            unsafe {
                FwpmEngineClose0(*h);
            }
            *h = 0 as HANDLE;
        }
        self.exclusive_active.store(false, Ordering::SeqCst);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BandwidthX Privileged Windows Service starting...");
    println!("IPC Address: {}", BANDWIDTHX_IPC_PIPE_NAME);

    let engine = Arc::new(ServiceWfpEngine::new());
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(BANDWIDTHX_IPC_PIPE_NAME)?;

    println!("Listening for BandwidthX desktop connections on Named Pipe...");

    loop {
        server.connect().await?;
        let engine_clone = Arc::clone(&engine);

        // Simple line/json protocol
        let mut buf = vec![0u8; 4096];
        if let Ok(n) = server.try_read(&mut buf) {
            if n > 0 {
                if let Ok(req) = serde_json::from_slice::<IpcRequest>(&buf[..n]) {
                    let resp = match req {
                        IpcRequest::Ping => IpcResponse::Pong,
                        IpcRequest::GetStatus => IpcResponse::Status(ServiceStatus {
                            is_running: true,
                            is_elevated: true,
                            is_service_connected: true,
                            exclusive_mode_active: engine_clone.exclusive_active.load(Ordering::SeqCst),
                            active_filter_count: 8,
                            version: "1.0.0".to_string(),
                        }),
                        IpcRequest::EnableExclusiveMode { target_exe_path, target_app_name, .. } => {
                            match engine_clone.enable_exclusive(&target_exe_path, &target_app_name) {
                                Ok(_) => IpcResponse::Ok { message: "Exclusive mode active".to_string() },
                                Err(e) => IpcResponse::Error { code: 1, message: e },
                            }
                        }
                        IpcRequest::DisableExclusiveMode => {
                            engine_clone.emergency_restore();
                            IpcResponse::Ok { message: "Exclusive mode disabled".to_string() }
                        }
                        IpcRequest::EmergencyRestore => {
                            engine_clone.emergency_restore();
                            IpcResponse::Ok { message: "Restored normal network".to_string() }
                        }
                        _ => IpcResponse::Ok { message: "Acknowledged".to_string() },
                    };

                    if let Ok(resp_bytes) = serde_json::to_vec(&resp) {
                        let _ = server.try_write(&resp_bytes);
                    }
                }
            }
        }

        // Recreate pipe instance for next connection
        server = ServerOptions::new().create(BANDWIDTHX_IPC_PIPE_NAME)?;
    }
}
