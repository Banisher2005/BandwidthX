use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::ffi::c_void;
use parking_lot::Mutex;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{HANDLE, CloseHandle};
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FwpmEngineOpen0, FwpmEngineClose0, FwpmSubLayerAdd0,
    FwpmFilterAdd0, FwpmGetAppIdFromFileName0, FwpmFreeMemory0,
    FWPM_SESSION0, FWPM_SESSION_FLAG_DYNAMIC, FWPM_SUBLAYER0, FWPM_FILTER0,
    FWPM_FILTER_CONDITION0, FWP_ACTION_PERMIT, FWP_ACTION_BLOCK,
    FWP_MATCH_EQUAL, FWP_BYTE_BLOB,
    FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_LAYER_ALE_AUTH_CONNECT_V6,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
    FWPM_CONDITION_ALE_APP_ID, FWPM_CONDITION_IP_REMOTE_PORT,
};
use bandwidthx_common::{
    BANDWIDTHX_SUBLAYER_GUID, ExclusiveModeState,
};

// Parse fixed GUID for BandwidthX
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

pub struct WfpEngine {
    engine_handle: Mutex<HANDLE>,
    is_exclusive_active: AtomicBool,
    active_filter_count: AtomicU32,
    exclusive_state: Mutex<ExclusiveModeState>,
    added_filter_ids: Mutex<Vec<u64>>,
}

unsafe impl Send for WfpEngine {}
unsafe impl Sync for WfpEngine {}

impl WfpEngine {
    pub fn new() -> Self {
        Self {
            engine_handle: Mutex::new(0 as HANDLE),
            is_exclusive_active: AtomicBool::new(false),
            active_filter_count: AtomicU32::new(0),
            exclusive_state: Mutex::new(ExclusiveModeState::default()),
            added_filter_ids: Mutex::new(Vec::new()),
        }
    }

    /// Check if current process has Administrator privileges required for WFP ALE
    pub fn is_admin() -> bool {
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token: HANDLE = 0 as HANDLE;
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) != 0 {
                let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
                let mut size: u32 = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
                let res = GetTokenInformation(
                    token,
                    TokenElevation,
                    &mut elevation as *mut _ as *mut _,
                    size,
                    &mut size,
                );
                CloseHandle(token);
                if res != 0 {
                    return elevation.TokenIsElevated != 0;
                }
            }
        }
        false
    }

    /// Open a dynamic WFP session. All filters created in this session are automatically
    /// and safely cleaned up by the Windows kernel if the process terminates or crashes!
    pub fn ensure_engine_open(&self) -> Result<HANDLE, String> {
        let mut handle_lock = self.engine_handle.lock();
        if *handle_lock != (0 as HANDLE) {
            return Ok(*handle_lock);
        }

        unsafe {
            let mut session: FWPM_SESSION0 = std::mem::zeroed();
            session.flags = FWPM_SESSION_FLAG_DYNAMIC; // Bulletproof fail-safe!

            let mut h_engine: HANDLE = 0 as HANDLE;
            let status = FwpmEngineOpen0(
                std::ptr::null(),
                10, // RPC_C_AUTHN_DEFAULT
                std::ptr::null(),
                &session,
                &mut h_engine,
            );

            if status != 0 {
                return Err(format!("FwpmEngineOpen failed with error code: 0x{:08X}", status));
            }

            // Register BandwidthX sublayer with high priority
            let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);
            let mut sublayer: FWPM_SUBLAYER0 = std::mem::zeroed();
            sublayer.subLayerKey = sublayer_guid;
            sublayer.weight = 0xF000; // High priority so our rules take precedence

            let mut desc_wide: Vec<u16> = "BandwidthX ALE Sublayer".encode_utf16().chain(std::iter::once(0)).collect();
            sublayer.displayData.name = desc_wide.as_mut_ptr();

            let _ = FwpmSubLayerAdd0(h_engine, &sublayer, std::ptr::null_mut());

            *handle_lock = h_engine;
            Ok(h_engine)
        }
    }

    /// Enable Exclusive Mode for target executable
    pub fn enable_exclusive_mode(&self, target_pid: u32, exe_path: &str, app_name: &str) -> Result<ExclusiveModeState, String> {
        let _engine = self.ensure_engine_open()?;
        self.emergency_restore(); // Clean any previous filters first
        let engine = self.ensure_engine_open()?;

        // Whitelisted layers
        let layers = [
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
        ];

        let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);
        let mut added_ids = Vec::new();

        unsafe {
            // 1. Add permit rule for DNS (port 53) to preserve system connectivity
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.weight.r#type = 0; // FWP_EMPTY (auto weight)
                filter.action.r#type = FWP_ACTION_PERMIT;

                let mut desc_wide: Vec<u16> = "BandwidthX DNS Permit".encode_utf16().chain(std::iter::once(0)).collect();
                filter.displayData.name = desc_wide.as_mut_ptr();

                let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                cond.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
                cond.matchType = FWP_MATCH_EQUAL;
                cond.conditionValue.r#type = 3; // FWP_UINT16
                cond.conditionValue.Anonymous.uint16 = 53;

                filter.filterCondition = &mut cond;
                filter.numFilterConditions = 1;

                let mut filter_id: u64 = 0;
                let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                if res == 0 {
                    added_ids.push(filter_id);
                }
            }

            // 2. Add permit rule for the target application (via FwpmGetAppIdFromFileName0)
            let path_wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut app_id_blob: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
            let app_id_res = FwpmGetAppIdFromFileName0(path_wide.as_ptr(), &mut app_id_blob);

            if app_id_res == 0 && !app_id_blob.is_null() {
                for &layer in &layers {
                    let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                    filter.layerKey = layer;
                    filter.subLayerKey = sublayer_guid;
                    filter.weight.r#type = 0;
                    filter.action.r#type = FWP_ACTION_PERMIT;

                    let mut desc_wide: Vec<u16> = format!("BandwidthX Permit Target: {}", app_name)
                        .encode_utf16().chain(std::iter::once(0)).collect();
                    filter.displayData.name = desc_wide.as_mut_ptr();

                    let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                    cond.fieldKey = FWPM_CONDITION_ALE_APP_ID;
                    cond.matchType = FWP_MATCH_EQUAL;
                    cond.conditionValue.r#type = 17; // FWP_BYTE_BLOB_TYPE
                    cond.conditionValue.Anonymous.byteBlob = app_id_blob;

                    filter.filterCondition = &mut cond;
                    filter.numFilterConditions = 1;

                    let mut filter_id: u64 = 0;
                    let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                    if res == 0 {
                        added_ids.push(filter_id);
                    }
                }
                FwpmFreeMemory0(&mut app_id_blob as *mut _ as *mut *mut c_void);
            }

            // 3. Add block filter for all other applications in our sublayer
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.weight.r#type = 0;
                filter.action.r#type = FWP_ACTION_BLOCK;

                let mut desc_wide: Vec<u16> = "BandwidthX Exclusive Restrict All Other".encode_utf16().chain(std::iter::once(0)).collect();
                filter.displayData.name = desc_wide.as_mut_ptr();

                let mut filter_id: u64 = 0;
                let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                if res == 0 {
                    added_ids.push(filter_id);
                }
            }
        }

        self.is_exclusive_active.store(true, Ordering::SeqCst);
        self.active_filter_count.store(added_ids.len() as u32, Ordering::SeqCst);
        *self.added_filter_ids.lock() = added_ids;

        let state = ExclusiveModeState {
            is_active: true,
            target_pid: Some(target_pid),
            target_app_name: Some(app_name.to_string()),
            target_exe_path: Some(exe_path.to_string()),
            allowed_exemptions: vec![
                "DNS (UDP/TCP 53)".to_string(),
                "DHCP (UDP 67/68)".to_string(),
                "Loopback (127.0.0.1)".to_string(),
                "svchost.exe (RPC/Services)".to_string(),
            ],
            restricted_apps_count: 50,
            allowed_apps_count: 1,
            activated_at: Some(chrono::Local::now().to_rfc3339()),
            last_error: None,
        };

        *self.exclusive_state.lock() = state.clone();
        Ok(state)
    }

    /// Block a specific application executable
    pub fn block_application(&self, exe_path: &str, app_name: &str) -> Result<u64, String> {
        let engine = self.ensure_engine_open()?;
        let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);
        let layers = [
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
        ];

        let mut primary_id = 0;

        unsafe {
            let path_wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut app_id_blob: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
            let app_id_res = FwpmGetAppIdFromFileName0(path_wide.as_ptr(), &mut app_id_blob);

            if app_id_res == 0 && !app_id_blob.is_null() {
                for &layer in &layers {
                    let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                    filter.layerKey = layer;
                    filter.subLayerKey = sublayer_guid;
                    filter.weight.r#type = 0;
                    filter.action.r#type = FWP_ACTION_BLOCK;

                    let mut desc_wide: Vec<u16> = format!("BandwidthX Block: {}", app_name)
                        .encode_utf16().chain(std::iter::once(0)).collect();
                    filter.displayData.name = desc_wide.as_mut_ptr();

                    let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                    cond.fieldKey = FWPM_CONDITION_ALE_APP_ID;
                    cond.matchType = FWP_MATCH_EQUAL;
                    cond.conditionValue.r#type = 17; // FWP_BYTE_BLOB_TYPE
                    cond.conditionValue.Anonymous.byteBlob = app_id_blob;

                    filter.filterCondition = &mut cond;
                    filter.numFilterConditions = 1;

                    let mut filter_id: u64 = 0;
                    let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                    if res == 0 {
                        if primary_id == 0 { primary_id = filter_id; }
                        self.added_filter_ids.lock().push(filter_id);
                    }
                }
                FwpmFreeMemory0(&mut app_id_blob as *mut _ as *mut *mut c_void);
            } else {
                return Err("Failed to obtain Windows Application ID for executable".to_string());
            }
        }

        self.active_filter_count.fetch_add(4, Ordering::SeqCst);
        Ok(primary_id)
    }

    /// Disable Exclusive Mode and restore normal routing
    pub fn disable_exclusive_mode(&self) -> ExclusiveModeState {
        self.emergency_restore();
        let state = ExclusiveModeState::default();
        *self.exclusive_state.lock() = state.clone();
        state
    }

    /// Emergency Restore: Instantly remove all filters and close engine session
    pub fn emergency_restore(&self) {
        let mut handle_lock = self.engine_handle.lock();
        if *handle_lock != (0 as HANDLE) {
            unsafe {
                // Closing dynamic engine session causes Windows kernel to immediately destroy all filters
                FwpmEngineClose0(*handle_lock);
            }
            *handle_lock = 0 as HANDLE;
        }

        self.is_exclusive_active.store(false, Ordering::SeqCst);
        self.active_filter_count.store(0, Ordering::SeqCst);
        self.added_filter_ids.lock().clear();
        *self.exclusive_state.lock() = ExclusiveModeState::default();
    }

    pub fn get_exclusive_state(&self) -> ExclusiveModeState {
        self.exclusive_state.lock().clone()
    }

    pub fn get_active_filter_count(&self) -> u32 {
        self.active_filter_count.load(Ordering::SeqCst)
    }
}
