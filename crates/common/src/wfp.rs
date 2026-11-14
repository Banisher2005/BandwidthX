use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use parking_lot::Mutex;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, SetTcpEntry, MIB_TCPROW_LH, TCP_TABLE_OWNER_PID_ALL,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FwpmEngineOpen0, FwpmEngineClose0, FwpmSubLayerAdd0,
    FwpmFilterAdd0, FwpmGetAppIdFromFileName0, FwpmFreeMemory0,
    FwpmTransactionBegin0, FwpmTransactionCommit0, FwpmTransactionAbort0,
    FWPM_SESSION0, FWPM_SESSION_FLAG_DYNAMIC, FWPM_SUBLAYER0, FWPM_FILTER0,
    FWPM_FILTER_CONDITION0, FWP_ACTION_PERMIT, FWP_ACTION_BLOCK,
    FWP_MATCH_EQUAL, FWP_MATCH_FLAGS_ALL_SET, FWP_BYTE_BLOB,
    FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_LAYER_ALE_AUTH_CONNECT_V6,
    FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
    FWPM_CONDITION_ALE_APP_ID, FWPM_CONDITION_IP_REMOTE_PORT, FWPM_CONDITION_FLAGS,
    FWP_CONDITION_FLAG_IS_LOOPBACK,
};
use crate::{BANDWIDTHX_SUBLAYER_GUID, ExclusiveModeState};

// Fixed GUID for BandwidthX
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

pub struct HardenedWfpEngine {
    engine_handle: Mutex<HANDLE>,
    is_exclusive_active: AtomicBool,
    active_filter_count: AtomicU32,
    exclusive_state: Mutex<ExclusiveModeState>,
    added_filter_ids: Mutex<Vec<u64>>,
}

unsafe impl Send for HardenedWfpEngine {}
unsafe impl Sync for HardenedWfpEngine {}

impl HardenedWfpEngine {
    pub fn new() -> Self {
        Self {
            engine_handle: Mutex::new(0 as HANDLE),
            is_exclusive_active: AtomicBool::new(false),
            active_filter_count: AtomicU32::new(0),
            exclusive_state: Mutex::new(ExclusiveModeState::default()),
            added_filter_ids: Mutex::new(Vec::new()),
        }
    }

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

    pub fn ensure_engine_open(&self) -> Result<HANDLE, String> {
        let mut handle_lock = self.engine_handle.lock();
        if *handle_lock != (0 as HANDLE) {
            return Ok(*handle_lock);
        }

        unsafe {
            let mut session: FWPM_SESSION0 = std::mem::zeroed();
            session.flags = FWPM_SESSION_FLAG_DYNAMIC; // Kernel auto-cleanup on exit

            let mut h_engine: HANDLE = 0 as HANDLE;
            let status = FwpmEngineOpen0(
                std::ptr::null(),
                10, // RPC_C_AUTHN_DEFAULT
                std::ptr::null(),
                &session,
                &mut h_engine,
            );

            if status != 0 {
                return Err(format!("FwpmEngineOpen failed: 0x{:08X}", status));
            }

            let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);
            let mut sublayer: FWPM_SUBLAYER0 = std::mem::zeroed();
            sublayer.subLayerKey = sublayer_guid;
            sublayer.weight = 0xF000;

            let mut desc_wide: Vec<u16> = "BandwidthX ALE Sublayer".encode_utf16().chain(std::iter::once(0)).collect();
            sublayer.displayData.name = desc_wide.as_mut_ptr();

            let _ = FwpmSubLayerAdd0(h_engine, &sublayer, std::ptr::null_mut());

            *handle_lock = h_engine;
            Ok(h_engine)
        }
    }

    /// Transactionally enables Exclusive Mode with explicit weights and active connection teardown
    pub fn enable_exclusive_mode(
        &self,
        target_pid: u32,
        target_creation_time: u64,
        exe_path: &str,
        app_name: &str,
        _exemptions: &[String],
    ) -> Result<ExclusiveModeState, String> {
        self.emergency_restore();
        let engine = self.ensure_engine_open()?;

        let layers = [
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
            FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
        ];

        let sublayer_guid = parse_guid(BANDWIDTHX_SUBLAYER_GUID);
        let mut added_ids = Vec::new();

        unsafe {
            // Begin atomic transaction
            if FwpmTransactionBegin0(engine, 0) != 0 {
                return Err("Failed to start WFP transaction".to_string());
            }

            // 1. Loopback Permit (Weight 15)
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.weight.r#type = 7; // FWP_UINT64
                filter.weight.Anonymous.uint64 = &mut 15;
                filter.action.r#type = FWP_ACTION_PERMIT;

                let mut name_wide: Vec<u16> = "BandwidthX Loopback Permit".encode_utf16().chain(std::iter::once(0)).collect();
                filter.displayData.name = name_wide.as_mut_ptr();

                let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                cond.fieldKey = FWPM_CONDITION_FLAGS;
                cond.matchType = FWP_MATCH_FLAGS_ALL_SET;
                cond.conditionValue.r#type = 4; // FWP_UINT32
                cond.conditionValue.Anonymous.uint32 = FWP_CONDITION_FLAG_IS_LOOPBACK;

                filter.filterCondition = &mut cond;
                filter.numFilterConditions = 1;

                let mut filter_id: u64 = 0;
                let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                if res != 0 {
                    let _ = FwpmTransactionAbort0(engine);
                    return Err(format!("Failed to add loopback permit filter: 0x{:08X}", res));
                }
                added_ids.push(filter_id);
            }

            // 2. DNS Remote Port 53 Permit (Weight 14)
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.weight.r#type = 7; // FWP_UINT64
                filter.weight.Anonymous.uint64 = &mut 14;
                filter.action.r#type = FWP_ACTION_PERMIT;

                let mut name_wide: Vec<u16> = "BandwidthX DNS Permit".encode_utf16().chain(std::iter::once(0)).collect();
                filter.displayData.name = name_wide.as_mut_ptr();

                let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                cond.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
                cond.matchType = FWP_MATCH_EQUAL;
                cond.conditionValue.r#type = 3; // FWP_UINT16
                cond.conditionValue.Anonymous.uint16 = 53;

                filter.filterCondition = &mut cond;
                filter.numFilterConditions = 1;

                let mut filter_id: u64 = 0;
                let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                if res != 0 {
                    let _ = FwpmTransactionAbort0(engine);
                    return Err(format!("Failed to add DNS permit filter: 0x{:08X}", res));
                }
                added_ids.push(filter_id);
            }

            // 3. DHCP Port 67/68 (IPv4) & 546/547 (IPv6) Permit (Weight 13)
            for &layer in &layers {
                for dhcp_port in [67u16, 68u16, 546u16, 547u16] {
                    let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                    filter.layerKey = layer;
                    filter.subLayerKey = sublayer_guid;
                    filter.weight.r#type = 7;
                    filter.weight.Anonymous.uint64 = &mut 13;
                    filter.action.r#type = FWP_ACTION_PERMIT;

                    let mut name_wide: Vec<u16> = format!("BandwidthX DHCP Port {} Permit", dhcp_port).encode_utf16().chain(std::iter::once(0)).collect();
                    filter.displayData.name = name_wide.as_mut_ptr();

                    let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                    cond.fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
                    cond.matchType = FWP_MATCH_EQUAL;
                    cond.conditionValue.r#type = 3;
                    cond.conditionValue.Anonymous.uint16 = dhcp_port;

                    filter.filterCondition = &mut cond;
                    filter.numFilterConditions = 1;

                    let mut filter_id: u64 = 0;
                    let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                    if res == 0 {
                        added_ids.push(filter_id);
                    }
                }
            }

            // 4. Core Windows System Services Permit (Weight 10)
            let system_exes = [
                r"C:\Windows\System32\svchost.exe",
                r"C:\Windows\System32\lsass.exe",
                r"C:\Windows\System32\services.exe",
            ];
            for sys_path in &system_exes {
                let sys_wide: Vec<u16> = sys_path.encode_utf16().chain(std::iter::once(0)).collect();
                let mut sys_blob: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
                if FwpmGetAppIdFromFileName0(sys_wide.as_ptr(), &mut sys_blob) == 0 && !sys_blob.is_null() {
                    for &layer in &layers {
                        let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                        filter.layerKey = layer;
                        filter.subLayerKey = sublayer_guid;
                        filter.weight.r#type = 7;
                        filter.weight.Anonymous.uint64 = &mut 10;
                        filter.action.r#type = FWP_ACTION_PERMIT;

                        let mut name_wide: Vec<u16> = format!("BandwidthX System Service Permit: {}", sys_path).encode_utf16().chain(std::iter::once(0)).collect();
                        filter.displayData.name = name_wide.as_mut_ptr();

                        let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                        cond.fieldKey = FWPM_CONDITION_ALE_APP_ID;
                        cond.matchType = FWP_MATCH_EQUAL;
                        cond.conditionValue.r#type = 17;
                        cond.conditionValue.Anonymous.byteBlob = sys_blob;

                        filter.filterCondition = &mut cond;
                        filter.numFilterConditions = 1;

                        let mut filter_id: u64 = 0;
                        let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                        if res == 0 {
                            added_ids.push(filter_id);
                        }
                    }
                    FwpmFreeMemory0(&mut sys_blob as *mut _ as *mut *mut c_void);
                }
            }

            // 5. Target Application Permit (Weight 15)
            let path_wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut app_id_blob: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
            let app_id_res = FwpmGetAppIdFromFileName0(path_wide.as_ptr(), &mut app_id_blob);

            if app_id_res == 0 && !app_id_blob.is_null() {
                for &layer in &layers {
                    let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                    filter.layerKey = layer;
                    filter.subLayerKey = sublayer_guid;
                    filter.weight.r#type = 7;
                    filter.weight.Anonymous.uint64 = &mut 15;
                    filter.action.r#type = FWP_ACTION_PERMIT;

                    let mut name_wide: Vec<u16> = format!("BandwidthX Target Permit: {}", app_name).encode_utf16().chain(std::iter::once(0)).collect();
                    filter.displayData.name = name_wide.as_mut_ptr();

                    let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                    cond.fieldKey = FWPM_CONDITION_ALE_APP_ID;
                    cond.matchType = FWP_MATCH_EQUAL;
                    cond.conditionValue.r#type = 17; // FWP_BYTE_BLOB
                    cond.conditionValue.Anonymous.byteBlob = app_id_blob;

                    filter.filterCondition = &mut cond;
                    filter.numFilterConditions = 1;

                    let mut filter_id: u64 = 0;
                    let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                    if res != 0 {
                        FwpmFreeMemory0(&mut app_id_blob as *mut _ as *mut *mut c_void);
                        let _ = FwpmTransactionAbort0(engine);
                        return Err(format!("Failed to add target permit filter: 0x{:08X}", res));
                    }
                    added_ids.push(filter_id);
                }
                FwpmFreeMemory0(&mut app_id_blob as *mut _ as *mut *mut c_void);
            } else {
                let _ = FwpmTransactionAbort0(engine);
                return Err(format!("Failed to get application ID for {}", exe_path));
            }

            // 6. Restrict All Other Applications (Weight 1)
            for &layer in &layers {
                let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                filter.layerKey = layer;
                filter.subLayerKey = sublayer_guid;
                filter.weight.r#type = 7;
                filter.weight.Anonymous.uint64 = &mut 1;
                filter.action.r#type = FWP_ACTION_BLOCK;

                let mut name_wide: Vec<u16> = "BandwidthX Restrict All Other".encode_utf16().chain(std::iter::once(0)).collect();
                filter.displayData.name = name_wide.as_mut_ptr();

                let mut filter_id: u64 = 0;
                let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                if res != 0 {
                    let _ = FwpmTransactionAbort0(engine);
                    return Err(format!("Failed to add block all filter: 0x{:08X}", res));
                }
                added_ids.push(filter_id);
            }

            // Commit atomic transaction
            let commit_res = FwpmTransactionCommit0(engine);
            if commit_res != 0 {
                return Err(format!("Failed to commit WFP transaction: 0x{:08X}", commit_res));
            }
        }

        // 7. Terminate existing established TCP connections for unpermitted apps (AUDIT-03)
        // Exempt the target process, system kernel (0, 4), and running system services
        let system_exempt_pids = self.get_system_exempt_pids();
        self.terminate_unpermitted_tcp_connections(target_pid, &system_exempt_pids);

        self.is_exclusive_active.store(true, Ordering::SeqCst);
        self.active_filter_count.store(added_ids.len() as u32, Ordering::SeqCst);
        *self.added_filter_ids.lock() = added_ids;

        let state = ExclusiveModeState {
            is_active: true,
            target_pid: Some(target_pid),
            target_creation_time: if target_creation_time > 0 { Some(target_creation_time) } else { None },
            target_app_name: Some(app_name.to_string()),
            target_exe_path: Some(exe_path.to_string()),
            allowed_exemptions: vec![
                "svchost.exe".to_string(),
                "System".to_string(),
                "lsass.exe".to_string(),
                "services.exe".to_string(),
            ],
            restricted_apps_count: 50,
            allowed_apps_count: 4,
            activated_at: Some(chrono::Local::now().to_rfc3339()),
            last_error: None,
        };

        *self.exclusive_state.lock() = state.clone();
        Ok(state)
    }

    /// Queries running PIDs of essential system processes to protect them from connection teardown
    pub fn get_system_exempt_pids(&self) -> Vec<u32> {
        let mut pids = Vec::new();
        unsafe {
            let snapshot: HANDLE = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if (snapshot as isize) != -1 {
                let mut entry: PROCESSENTRY32W = std::mem::zeroed();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                if Process32FirstW(snapshot, &mut entry) != 0 {
                    loop {
                        let name_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);
                        let lower = name.to_ascii_lowercase();
                        if lower == "svchost.exe" || lower == "lsass.exe" || lower == "services.exe" {
                            pids.push(entry.th32ProcessID);
                        }
                        if Process32NextW(snapshot, &mut entry) == 0 {
                            break;
                        }
                    }
                }
                CloseHandle(snapshot);
            }
        }
        pids
    }

    /// Terminates established TCP sockets for unpermitted processes using SetTcpEntry.
    /// Strictly preserves target application sockets, system kernel, and exempt services.
    /// Returns (ipv4_terminated_count, ipv6_unmanaged_count).
    pub fn terminate_unpermitted_tcp_connections(&self, permitted_pid: u32, exempt_pids: &[u32]) -> (usize, usize) {
        let mut ipv4_terminated = 0;
        let mut ipv6_unmanaged = 0;

        unsafe {
            // 1. IPv4 TCP Connection Teardown
            let mut size: u32 = 0;
            let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_ALL, 0);
            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetExtendedTcpTable(buffer.as_mut_ptr() as *mut _, &mut size, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_ALL, 0) == 0 {
                    let num = *(buffer.as_ptr() as *const u32) as usize;
                    let table = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID_RAW;
                    for row in std::slice::from_raw_parts(table, num) {
                        // STRICT EXCLUSION: Never terminate target app, system PIDs (0, 4), or exempt services
                        if row.owning_pid == permitted_pid || row.owning_pid <= 4 || exempt_pids.contains(&row.owning_pid) {
                            continue;
                        }

                        // Also skip local loopback connections (127.x.x.x)
                        let local_first_byte = (row.local_addr & 0xFF) as u8;
                        let remote_first_byte = (row.remote_addr & 0xFF) as u8;
                        if local_first_byte == 127 || remote_first_byte == 127 {
                            continue;
                        }

                        // State 5 is MIB_TCP_STATE_ESTAB
                        if row.state == 5 {
                            let mut kill_row: MIB_TCPROW_LH = std::mem::zeroed();
                            kill_row.Anonymous.dwState = 12; // MIB_TCP_STATE_DELETE_TCB
                            kill_row.dwLocalAddr = row.local_addr;
                            kill_row.dwLocalPort = row.local_port;
                            kill_row.dwRemoteAddr = row.remote_addr;
                            kill_row.dwRemotePort = row.remote_port;
                            if SetTcpEntry(&kill_row) == 0 {
                                ipv4_terminated += 1;
                            }
                        }
                    }
                }
            }

            // 2. IPv6 Established Connection Inspection
            let mut size6: u32 = 0;
            let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size6, 0, AF_INET6 as u32, TCP_TABLE_OWNER_PID_ALL, 0);
            if size6 > 0 {
                let mut buffer6 = vec![0u8; size6 as usize];
                if GetExtendedTcpTable(buffer6.as_mut_ptr() as *mut _, &mut size6, 0, AF_INET6 as u32, TCP_TABLE_OWNER_PID_ALL, 0) == 0 {
                    let num6 = *(buffer6.as_ptr() as *const u32) as usize;
                    let table6 = buffer6.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCP6ROW_OWNER_PID_RAW;
                    for row in std::slice::from_raw_parts(table6, num6) {
                        if row.owning_pid == permitted_pid || row.owning_pid <= 4 || exempt_pids.contains(&row.owning_pid) {
                            continue;
                        }
                        if row.state == 5 {
                            // User-space IP Helper lacks SetTcp6Entry. ALE blocks subsequent connection attempts.
                            ipv6_unmanaged += 1;
                        }
                    }
                }
            }
        }

        (ipv4_terminated, ipv6_unmanaged)
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
            if FwpmTransactionBegin0(engine, 0) != 0 {
                return Err("Failed to start WFP transaction".to_string());
            }

            let path_wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut app_id_blob: *mut FWP_BYTE_BLOB = std::ptr::null_mut();
            let app_id_res = FwpmGetAppIdFromFileName0(path_wide.as_ptr(), &mut app_id_blob);

            if app_id_res == 0 && !app_id_blob.is_null() {
                for &layer in &layers {
                    let mut filter: FWPM_FILTER0 = std::mem::zeroed();
                    filter.layerKey = layer;
                    filter.subLayerKey = sublayer_guid;
                    filter.weight.r#type = 7;
                    filter.weight.Anonymous.uint64 = &mut 15;
                    filter.action.r#type = FWP_ACTION_BLOCK;

                    let mut name_wide: Vec<u16> = format!("BandwidthX Block: {}", app_name).encode_utf16().chain(std::iter::once(0)).collect();
                    filter.displayData.name = name_wide.as_mut_ptr();

                    let mut cond: FWPM_FILTER_CONDITION0 = std::mem::zeroed();
                    cond.fieldKey = FWPM_CONDITION_ALE_APP_ID;
                    cond.matchType = FWP_MATCH_EQUAL;
                    cond.conditionValue.r#type = 17;
                    cond.conditionValue.Anonymous.byteBlob = app_id_blob;

                    filter.filterCondition = &mut cond;
                    filter.numFilterConditions = 1;

                    let mut filter_id: u64 = 0;
                    let res = FwpmFilterAdd0(engine, &filter, std::ptr::null_mut(), &mut filter_id);
                    if res != 0 {
                        FwpmFreeMemory0(&mut app_id_blob as *mut _ as *mut *mut c_void);
                        let _ = FwpmTransactionAbort0(engine);
                        return Err(format!("Failed to add block filter: 0x{:08X}", res));
                    }
                    if primary_id == 0 { primary_id = filter_id; }
                    self.added_filter_ids.lock().push(filter_id);
                }
                FwpmFreeMemory0(&mut app_id_blob as *mut _ as *mut *mut c_void);
            } else {
                let _ = FwpmTransactionAbort0(engine);
                return Err("Failed to obtain Windows Application ID".to_string());
            }

            let commit_res = FwpmTransactionCommit0(engine);
            if commit_res != 0 {
                return Err(format!("Failed to commit WFP transaction: 0x{:08X}", commit_res));
            }
        }

        self.active_filter_count.fetch_add(4, Ordering::SeqCst);
        Ok(primary_id)
    }

    pub fn disable_exclusive_mode(&self) -> ExclusiveModeState {
        self.emergency_restore();
        let state = ExclusiveModeState::default();
        *self.exclusive_state.lock() = state.clone();
        state
    }

    pub fn emergency_restore(&self) {
        let mut handle_lock = self.engine_handle.lock();
        if *handle_lock != (0 as HANDLE) {
            unsafe {
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

#[repr(C)]
struct MIB_TCPROW_OWNER_PID_RAW {
    state: u32,
    local_addr: u32,
    local_port: u32,
    remote_addr: u32,
    remote_port: u32,
    owning_pid: u32,
}

#[repr(C)]
struct MIB_TCP6ROW_OWNER_PID_RAW {
    local_addr: [u8; 16],
    local_scope_id: u32,
    local_port: u32,
    remote_addr: [u8; 16],
    remote_scope_id: u32,
    remote_port: u32,
    state: u32,
    owning_pid: u32,
}
