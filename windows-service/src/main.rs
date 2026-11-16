use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, FILETIME, LocalFree, GetLastError};
use windows_sys::Win32::Security::{
    Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertSidToStringSidW},
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY, SECURITY_ATTRIBUTES,
    CheckTokenMembership, AllocateAndInitializeSid, FreeSid, RevertToSelf,
    GetKernelObjectSecurity, DACL_SECURITY_INFORMATION,
    GetSecurityDescriptorDacl, GetAce, EqualSid,
    ACL, ACCESS_ALLOWED_ACE,
    SECURITY_NT_AUTHORITY,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, ImpersonateNamedPipeClient};
use windows_sys::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, GetProcessTimes, WaitForSingleObject,
};
use windows_sys::Win32::System::Services::{
    OpenSCManagerW, CreateServiceW, OpenServiceW, DeleteService, CloseServiceHandle,
    StartServiceW, ControlService, QueryServiceStatusEx, QueryServiceConfig2W,
    StartServiceCtrlDispatcherW, RegisterServiceCtrlHandlerExW, SetServiceStatus,
    SERVICE_TABLE_ENTRYW, SERVICE_STATUS_HANDLE,
    SC_MANAGER_ALL_ACCESS, SERVICE_WIN32_OWN_PROCESS, SERVICE_AUTO_START,
    SERVICE_ERROR_NORMAL, SERVICE_ALL_ACCESS, SERVICE_STOP,
    SERVICE_CONTROL_STOP, SERVICE_CONTROL_SHUTDOWN, SERVICE_STATUS, SERVICE_STATUS_PROCESS, SC_STATUS_PROCESS_INFO,
    SERVICE_STOPPED, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STOP_PENDING,
    SERVICE_ACCEPT_STOP, SERVICE_ACCEPT_SHUTDOWN,
    ChangeServiceConfig2W, SERVICE_CONFIG_FAILURE_ACTIONS, SERVICE_FAILURE_ACTIONSW,
    SC_ACTION, SC_ACTION_RESTART,
};

const DELETE: u32 = 0x00010000;
const SYNCHRONIZE: u32 = 0x00100000;
const WAIT_TIMEOUT: u32 = 0x00000102;
const SECURITY_BUILTIN_DOMAIN_RID: u32 = 0x00000020;
const DOMAIN_ALIAS_RID_ADMINS: u32 = 0x00000220;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;

use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus
};

// ------------------------------------------------------------------------------------------------
// Global State for Windows SCM Dispatcher & Service Control
// ------------------------------------------------------------------------------------------------

static SERVICE_STATUS_HANDLE_RAW: AtomicIsize = AtomicIsize::new(0);
static SHUTDOWN_TX: parking_lot::Mutex<Option<tokio::sync::broadcast::Sender<()>>> = parking_lot::Mutex::new(None);
static ACTIVE_ENGINE: parking_lot::RwLock<Option<Arc<HardenedWfpEngine>>> = parking_lot::RwLock::new(None);

/// Updates the Windows Service Control Manager with current lifecycle state
fn update_service_status(state: u32, win32_exit_code: u32, check_point: u32, wait_hint: u32) {
    let raw = SERVICE_STATUS_HANDLE_RAW.load(Ordering::SeqCst);
    if raw != 0 {
        let handle = raw as SERVICE_STATUS_HANDLE;
        let mut status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: state,
            dwControlsAccepted: match state {
                SERVICE_RUNNING => SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
                _ => 0,
            },
            dwWin32ExitCode: win32_exit_code,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: check_point,
            dwWaitHint: wait_hint,
        };
        unsafe {
            SetServiceStatus(handle, &mut status);
        }
    }
}

/// Handler callback for Windows SCM control requests.
/// Strictly non-blocking: never performs lengthy WFP operations directly.
/// Signals the runtime context to perform graceful cleanup and returns immediately.
unsafe extern "system" fn service_control_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut c_void,
    _context: *mut c_void,
) -> u32 {
    match control {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            eprintln!("[SCM] Received service control stop/shutdown signal. Transitioning to STOP_PENDING...");
            update_service_status(SERVICE_STOP_PENDING, 0, 1, 10000);

            // Signal Tokio server loop to perform bounded graceful shutdown in service context
            if let Some(tx) = SHUTDOWN_TX.lock().as_ref() {
                let _ = tx.send(());
            }

            0 // NO_ERROR returned immediately to SCM
        }
        _ => 120, // ERROR_CALL_NOT_IMPLEMENTED
    }
}

// ------------------------------------------------------------------------------------------------
// Process Identity Validation with Lifetime Handle Pinning & Revalidation
// ------------------------------------------------------------------------------------------------

/// Holds an open kernel handle to the validated process.
/// As long as this handle remains open, the Windows kernel cannot destroy the EPROCESS
/// structure, guaranteeing that the PID cannot be recycled before policy enforcement.
pub struct ValidatedProcess {
    pub handle: HANDLE,
    pub pid: u32,
    pub path: String,
    pub creation_time: u64,
}

impl Drop for ValidatedProcess {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

impl ValidatedProcess {
    /// Revalidates that the target process is still actively executing immediately before WFP enforcement
    pub fn revalidate_and_enforce<F, R>(&self, enforce_fn: F) -> Result<R, String>
    where
        F: FnOnce(&str, u64) -> Result<R, String>,
    {
        unsafe {
            // WaitForSingleObject with 0 timeout checks if the process object is signaled (terminated)
            let wait_res = WaitForSingleObject(self.handle, 0);
            if wait_res != WAIT_TIMEOUT {
                return Err(format!(
                    "Target process PID {} terminated or signaled prior to WFP filter commitment. Policy aborted.",
                    self.pid
                ));
            }
        }
        enforce_fn(&self.path, self.creation_time)
    }
}

/// Authoritatively validates a running process by opening a kernel handle,
/// checking for a nonzero creation timestamp, verifying image path identity,
/// and returning a `ValidatedProcess` that pins the PID against recycling.
fn validate_running_process(pid: u32, claimed_creation_time: u64, claimed_path: &str) -> Result<ValidatedProcess, String> {
    if claimed_path.is_empty() {
        return Err("Executable path cannot be empty".to_string());
    }

    if pid == 0 || pid == 4 {
        return Err("Cannot target system kernel process (PID 0 or 4)".to_string());
    }

    if claimed_creation_time == 0 {
        return Err("Process creation timestamp must be nonzero. Unverified or stale process identities are rejected.".to_string());
    }

    unsafe {
        // Open process handle with QUERY_LIMITED_INFORMATION and SYNCHRONIZE
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return Err(format!(
                "Process PID {} is not running or accessible: {}",
                pid,
                std::io::Error::last_os_error()
            ));
        }

        // 1. Validate process creation timestamp using consistent FILETIME comparison
        let mut creation_time: FILETIME = std::mem::zeroed();
        let mut exit_time: FILETIME = std::mem::zeroed();
        let mut kernel_time: FILETIME = std::mem::zeroed();
        let mut user_time: FILETIME = std::mem::zeroed();

        if GetProcessTimes(handle, &mut creation_time, &mut exit_time, &mut kernel_time, &mut user_time) == 0 {
            CloseHandle(handle);
            return Err(format!("Failed to retrieve process lifetime metrics for PID {}", pid));
        }

        let actual_ts = ((creation_time.dwHighDateTime as u64) << 32) | (creation_time.dwLowDateTime as u64);
        if actual_ts != claimed_creation_time {
            CloseHandle(handle);
            return Err(format!(
                "PID reuse detected: PID {} was recycled (expected timestamp {}, actual is {})",
                pid, claimed_creation_time, actual_ts
            ));
        }

        // 2. Query full image name on disk and verify against claimed path
        let mut path_buf = [0u16; 1024];
        let mut size: u32 = path_buf.len() as u32;
        let success = QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size);

        if success == 0 || size == 0 {
            CloseHandle(handle);
            return Err(format!("Failed to query process image name for PID {}", pid));
        }

        let actual_path = String::from_utf16_lossy(&path_buf[..size as usize]);
        if !actual_path.eq_ignore_ascii_case(claimed_path) {
            CloseHandle(handle);
            return Err(format!(
                "Process verification failed: PID {} image is '{}', but request claimed '{}'",
                pid, actual_path, claimed_path
            ));
        }

        Ok(ValidatedProcess {
            handle,
            pid,
            path: actual_path,
            creation_time: actual_ts,
        })
    }
}

// ------------------------------------------------------------------------------------------------
// Fail-Closed Named Pipe Security & Client Authorization (Phase 1)
// ------------------------------------------------------------------------------------------------

/// Creates a Windows SECURITY_ATTRIBUTES structure from an SDDL string
/// restricting pipe access strictly to Builtin Administrators (BA) and LocalSystem (SY).
fn create_admin_security_attributes() -> Option<(SECURITY_ATTRIBUTES, *mut c_void)> {
    unsafe {
        let sddl = "D:(A;;GA;;;BA)(A;;GA;;;SY)\0";
        let sddl_wide: Vec<u16> = sddl.encode_utf16().collect();
        let mut p_sd: *mut c_void = std::ptr::null_mut();
        let mut sd_size: u32 = 0;

        let res = ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_wide.as_ptr(),
            1, // SDDL_REVISION_1
            &mut p_sd,
            &mut sd_size,
        );

        if res != 0 && !p_sd.is_null() {
            let mut sa: SECURITY_ATTRIBUTES = std::mem::zeroed();
            sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
            sa.lpSecurityDescriptor = p_sd;
            sa.bInheritHandle = 0;
            Some((sa, p_sd))
        } else {
            None
        }
    }
}

/// Inspects the effective DACL returned by GetKernelObjectSecurity,
/// parses the security descriptor and validates its actual ACE entries,
/// rejects unexpected or excessively permissive ACEs, and verifies
/// that BOTH Builtin Administrators and LocalSystem permissions are present.
/// Fails closed on any parsing or validation failure.
fn verify_pipe_security(pipe_handle: HANDLE) -> Result<(), std::io::Error> {
    unsafe {
        let mut needed: u32 = 0;
        let _ = GetKernelObjectSecurity(
            pipe_handle,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
        if needed == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Named Pipe DACL verification failed: descriptor length is zero",
            ));
        }

        let mut sd_buf = vec![0u8; needed as usize];
        let ok = GetKernelObjectSecurity(
            pipe_handle,
            DACL_SECURITY_INFORMATION,
            sd_buf.as_mut_ptr() as *mut c_void,
            needed,
            &mut needed,
        );
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }

        let p_sd = sd_buf.as_ptr() as *mut c_void;
        let mut dacl_present: i32 = 0;
        let mut p_dacl: *mut ACL = std::ptr::null_mut();
        let mut dacl_defaulted: i32 = 0;

        let get_dacl_res = GetSecurityDescriptorDacl(
            p_sd,
            &mut dacl_present,
            &mut p_dacl,
            &mut dacl_defaulted,
        );

        if get_dacl_res == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Failed to retrieve DACL from security descriptor: {}", std::io::Error::last_os_error()),
            ));
        }

        if dacl_present == 0 || p_dacl.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "DACL verification failed: DACL is not present or NULL (grants unrestricted access)",
            ));
        }

        let acl = &*p_dacl;
        let ace_count = acl.AceCount as u32;
        if ace_count == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "DACL verification failed: DACL contains 0 ACE entries",
            ));
        }

        // Initialize well-known SIDs for comparison
        let mut nt_authority = SECURITY_NT_AUTHORITY;
        let mut admin_sid: *mut c_void = std::ptr::null_mut();
        let sid_admin_ok = AllocateAndInitializeSid(
            &mut nt_authority,
            2,
            SECURITY_BUILTIN_DOMAIN_RID,
            DOMAIN_ALIAS_RID_ADMINS,
            0, 0, 0, 0, 0, 0,
            &mut admin_sid,
        );

        let mut system_sid: *mut c_void = std::ptr::null_mut();
        let sid_system_ok = AllocateAndInitializeSid(
            &mut nt_authority,
            1,
            18, // SECURITY_LOCAL_SYSTEM_RID
            0, 0, 0, 0, 0, 0, 0,
            &mut system_sid,
        );

        if sid_admin_ok == 0 || sid_system_ok == 0 || admin_sid.is_null() || system_sid.is_null() {
            if !admin_sid.is_null() { FreeSid(admin_sid); }
            if !system_sid.is_null() { FreeSid(system_sid); }
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Failed to allocate baseline validation SIDs",
            ));
        }

        let mut has_admin_allowed = false;
        let mut has_system_allowed = false;

        for i in 0..ace_count {
            let mut p_ace: *mut c_void = std::ptr::null_mut();
            if GetAce(p_dacl, i, &mut p_ace) == 0 || p_ace.is_null() {
                FreeSid(admin_sid);
                FreeSid(system_sid);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Failed to retrieve ACE index {}", i),
                ));
            }

            let ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
            if ace.Header.AceType != ACCESS_ALLOWED_ACE_TYPE {
                FreeSid(admin_sid);
                FreeSid(system_sid);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Rejecting unexpected ACE type {} at index {}", ace.Header.AceType, i),
                ));
            }

            let ace_sid = &ace.SidStart as *const u32 as *mut c_void;
            if EqualSid(ace_sid, admin_sid) != 0 {
                has_admin_allowed = true;
            } else if EqualSid(ace_sid, system_sid) != 0 {
                has_system_allowed = true;
            } else {
                let mut sid_str_ptr: *mut u16 = std::ptr::null_mut();
                let _ = ConvertSidToStringSidW(ace_sid, &mut sid_str_ptr);
                let sid_str = if !sid_str_ptr.is_null() {
                    let len = (0..).take_while(|&idx| *sid_str_ptr.add(idx) != 0).count();
                    let s = String::from_utf16_lossy(std::slice::from_raw_parts(sid_str_ptr, len));
                    LocalFree(sid_str_ptr as *mut c_void);
                    s
                } else {
                    "Unknown SID".to_string()
                };

                FreeSid(admin_sid);
                FreeSid(system_sid);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("DACL security violation: unexpected or overly permissive ACE granted to SID {}", sid_str),
                ));
            }
        }

        FreeSid(admin_sid);
        FreeSid(system_sid);

        if !has_admin_allowed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "DACL verification failed: Builtin Administrators ACE is missing",
            ));
        }

        if !has_system_allowed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "DACL verification failed: LocalSystem ACE is missing",
            ));
        }

        Ok(())
    }
}

/// Instantiates the secured Named Pipe server with explicit Administrator/LocalSystem DACL.
/// Fails closed if SDDL conversion, secure creation, or DACL validation fails (NO unsecured fallback).
fn create_named_pipe_server(first_instance: bool) -> Result<NamedPipeServer, std::io::Error> {
    let mut options = ServerOptions::new();
    options.first_pipe_instance(first_instance);
    options.max_instances(8);

    let (mut sa, p_sd) = create_admin_security_attributes()
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "FATAL: Failed to generate secure SDDL security descriptor for Named Pipe"
        ))?;

    let res = unsafe {
        options.create_with_security_attributes_raw(
            BANDWIDTHX_IPC_PIPE_NAME,
            &mut sa as *mut _ as *mut c_void,
        )
    };

    unsafe {
        LocalFree(p_sd);
    }

    let server = res?;
    verify_pipe_security(server.as_raw_handle() as HANDLE)?;

    Ok(server)
}

/// RAII guard ensuring RevertToSelf is ALWAYS invoked on every execution and error path
struct ImpersonationGuard {
    active: bool,
}

impl ImpersonationGuard {
    fn new(pipe_handle: HANDLE) -> Result<Self, String> {
        let res = unsafe { ImpersonateNamedPipeClient(pipe_handle) };
        if res == 0 {
            Err(format!(
                "Failed to impersonate named pipe client: {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(Self { active: true })
        }
    }
}

impl Drop for ImpersonationGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                RevertToSelf();
            }
        }
    }
}

/// Comprehensive client authentication & authorization.
/// Validates both:
/// 1. Windows Security Reference Monitor check via `ImpersonateNamedPipeClient` & `CheckTokenMembership` (BUILTIN\Administrators)
/// 2. Client process token elevation check (`TokenElevation`)
/// Guarantees that `RevertToSelf()` runs on every return path via `ImpersonationGuard`.
fn authorize_client(pipe_handle: HANDLE) -> Result<u32, String> {
    unsafe {
        // 1. Get client PID
        let mut client_pid: u32 = 0;
        if GetNamedPipeClientProcessId(pipe_handle, &mut client_pid) == 0 {
            return Err("Unable to query named pipe client PID".to_string());
        }

        // 2. Impersonate client with RAII guard
        let guard = ImpersonationGuard::new(pipe_handle)?;

        // 3. Verify membership in BUILTIN\Administrators
        let mut nt_authority = SECURITY_NT_AUTHORITY;
        let mut admin_sid: *mut c_void = std::ptr::null_mut();
        let sid_ok = AllocateAndInitializeSid(
            &mut nt_authority,
            2,
            SECURITY_BUILTIN_DOMAIN_RID,
            DOMAIN_ALIAS_RID_ADMINS,
            0, 0, 0, 0, 0, 0,
            &mut admin_sid,
        );

        let mut is_admin: i32 = 0;
        if sid_ok != 0 && !admin_sid.is_null() {
            let _ = CheckTokenMembership(0 as HANDLE, admin_sid, &mut is_admin);
            FreeSid(admin_sid);
        }

        // Drop guard explicitly before checking process token
        drop(guard);

        if is_admin == 0 {
            return Err("Client identity is not an authorized Administrator or LocalSystem".to_string());
        }

        // 4. Additionally verify client process token elevation
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, client_pid);
        if handle.is_null() {
            return Err(format!("Client PID {} inaccessible: {}", client_pid, std::io::Error::last_os_error()));
        }

        let mut token: HANDLE = 0 as HANDLE;
        if OpenProcessToken(handle, TOKEN_QUERY, &mut token) == 0 {
            CloseHandle(handle);
            return Err("Unable to inspect client process token".to_string());
        }

        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut ret_len: u32 = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let elevated = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            ret_len,
            &mut ret_len,
        );

        CloseHandle(token);
        CloseHandle(handle);

        if elevated == 0 || elevation.TokenIsElevated == 0 {
            return Err("Client process token is not elevated".to_string());
        }

        Ok(client_pid)
    }
}

// ------------------------------------------------------------------------------------------------
// Windows Service Control Manager (SCM) Lifecycle & ServiceMain (Phase 2)
// ------------------------------------------------------------------------------------------------

/// Entrypoint invoked by Windows SCM when the service is started
unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    let service_name_wide: Vec<u16> = "BandwidthX\0".encode_utf16().collect();
    let status_handle = RegisterServiceCtrlHandlerExW(
        service_name_wide.as_ptr(),
        Some(service_control_handler),
        std::ptr::null_mut(),
    );

    if status_handle.is_null() {
        return;
    }
    SERVICE_STATUS_HANDLE_RAW.store(status_handle as isize, Ordering::SeqCst);

    // 1. Report SERVICE_START_PENDING
    update_service_status(SERVICE_START_PENDING, 0, 1, 5000);

    // 2. Build Tokio multi-thread runtime
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(_) => {
            update_service_status(SERVICE_STOPPED, 1, 0, 0);
            return;
        }
    };

    // 3. Execute service inside Tokio runtime
    runtime.block_on(async {
        let (shutdown_send, shutdown_recv) = tokio::sync::broadcast::channel::<()>(1);
        *SHUTDOWN_TX.lock() = Some(shutdown_send);

        let engine = Arc::new(HardenedWfpEngine::new());
        *ACTIVE_ENGINE.write() = Some(Arc::clone(&engine));

        // Report SERVICE_RUNNING to SCM
        update_service_status(SERVICE_RUNNING, 0, 0, 0);

        // Run authoritative IPC server loop until shutdown signal arrives
        if let Err(e) = run_ipc_server(Arc::clone(&engine), shutdown_recv).await {
            eprintln!("[Service] IPC server loop encountered error: {}", e);
        }

        // Bounded graceful cleanup in the service runtime execution context
        update_service_status(SERVICE_STOP_PENDING, 0, 2, 5000);
        eprintln!("[Service] Performing bounded emergency WFP purge in service execution context...");
        let restore_engine = Arc::clone(&engine);
        let restore_res = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                restore_engine.emergency_restore();
            })
        ).await;

        if let Err(_) = restore_res {
            eprintln!("[Service] Warning: Emergency restore timed out during service shutdown");
        }
    });

    // 4. Report SERVICE_STOPPED only after cleanup completes
    update_service_status(SERVICE_STOPPED, 0, 0, 0);
    SERVICE_STATUS_HANDLE_RAW.store(0, Ordering::SeqCst);
}

/// Asynchronous IPC server loop implementing the corrected authorization ordering:
/// 1. Accept client connection.
/// 2. Read initial bounded IPC frame.
/// 3. Impersonate client associated with received message.
/// 4. Validate administrator group membership.
/// 5. Validate client process token.
/// 6. Revert impersonation on every execution path.
/// 7. Only then execute request.
async fn run_ipc_server(
    engine: Arc<HardenedWfpEngine>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut server = create_named_pipe_server(true)?;

    loop {
        // Wait for incoming client connection or shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                eprintln!("[IPC] Shutdown signal received. Exiting IPC server loop.");
                break;
            }
            conn_res = server.connect() => {
                if let Err(e) = conn_res {
                    eprintln!("[IPC] Client connect failed: {}. Continuing...", e);
                    server = create_named_pipe_server(false)?;
                    continue;
                }
            }
        }

        let engine_clone = Arc::clone(&engine);
        let raw_handle = server.as_raw_handle() as HANDLE;

        // 1. Read the initial bounded IPC frame (header + payload)
        let mut len_buf = [0u8; 4];
        if let Err(e) = server.read_exact(&mut len_buf).await {
            eprintln!("[IPC] Client disconnected or failed to read header: {}", e);
            server = create_named_pipe_server(false)?;
            continue;
        }

        let frame_len = u32::from_be_bytes(len_buf) as usize;
        if frame_len == 0 || frame_len > 1024 * 1024 {
            eprintln!("[IPC] Rejecting invalid frame length: {} bytes", frame_len);
            let resp = IpcResponse::Error {
                code: 400,
                message: format!("Invalid frame length: {} (max 1 MB)", frame_len),
            };
            if let Ok(resp_frame) = encode_frame(&resp) {
                let _ = server.write_all(&resp_frame).await;
            }
            server = create_named_pipe_server(false)?;
            continue;
        }

        let mut payload_buf = vec![0u8; frame_len];
        if let Err(e) = server.read_exact(&mut payload_buf).await {
            eprintln!("[IPC] Failed to read full payload ({} bytes): {}", frame_len, e);
            server = create_named_pipe_server(false)?;
            continue;
        }

        let mut full_frame = Vec::with_capacity(4 + frame_len);
        full_frame.extend_from_slice(&len_buf);
        full_frame.extend_from_slice(&payload_buf);

        let req: IpcRequest = match decode_frame::<IpcRequest>(&full_frame) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[IPC] Malformed request frame: {}", e);
                let resp = IpcResponse::Error {
                    code: 400,
                    message: format!("Malformed request: {}", e),
                };
                if let Ok(resp_frame) = encode_frame(&resp) {
                    let _ = server.write_all(&resp_frame).await;
                }
                server = create_named_pipe_server(false)?;
                continue;
            }
        };

        // 2. Impersonate client associated with received message, validate admin membership, and process token
        let client_auth = authorize_client(raw_handle);

        let resp: IpcResponse = match client_auth {
            Err(auth_err) => {
                eprintln!("⚠ Unauthorized IPC connection rejected: {}", auth_err);
                IpcResponse::Error {
                    code: 403,
                    message: format!("Access Denied: {}", auth_err),
                }
            }
            Ok(_client_pid) => {
                // 3. ONLY THEN execute the request!
                match req {
                    IpcRequest::Ping => IpcResponse::Pong,
                    IpcRequest::GetStatus => {
                        let is_elevated = HardenedWfpEngine::is_admin();
                        let exclusive = engine_clone.get_exclusive_state().is_active;
                        let count = engine_clone.get_active_filter_count();
                        IpcResponse::Status(ServiceStatus {
                            is_running: true,
                            is_elevated,
                            is_service_connected: true,
                            exclusive_mode_active: exclusive,
                            active_filter_count: count,
                            version: "2.3.0".to_string(),
                        })
                    }
                    IpcRequest::GetExclusiveState => {
                        IpcResponse::ExclusiveState(engine_clone.get_exclusive_state())
                    }
                    IpcRequest::EnableExclusiveMode {
                        target_pid,
                        target_creation_time,
                        target_exe_path,
                        target_app_name,
                        exemptions,
                    } => {
                        // Authoritative validation of running target process
                        match validate_running_process(target_pid, target_creation_time, &target_exe_path) {
                            Ok(validated_process) => {
                                // Revalidate immediately before WFP filter commitment
                                let enforce_res = validated_process.revalidate_and_enforce(|path, creation_time| {
                                    engine_clone.enable_exclusive_mode(
                                        target_pid,
                                        creation_time,
                                        path,
                                        &target_app_name,
                                        &exemptions,
                                    )
                                });

                                match enforce_res {
                                    Ok(_) => IpcResponse::Ok {
                                        message: format!("Exclusive Mode enabled for {} (PID {})", target_app_name, target_pid),
                                    },
                                    Err(e) => IpcResponse::Error { code: 1, message: e },
                                }
                            }
                            Err(e) => IpcResponse::Error { code: 2, message: e },
                        }
                    }
                    IpcRequest::DisableExclusiveMode => {
                        engine_clone.disable_exclusive_mode();
                        IpcResponse::Ok { message: "Exclusive Mode disabled".to_string() }
                    }
                    IpcRequest::ApplyRule { rule } => {
                        if rule.rule_type == bandwidthx_common::RuleType::Block && rule.enabled {
                            match engine_clone.block_application(&rule.executable_path, &rule.app_name) {
                                Ok(_) => IpcResponse::Ok { message: format!("Blocked {}", rule.app_name) },
                                Err(e) => IpcResponse::Error { code: 3, message: e },
                            }
                        } else {
                            IpcResponse::Ok { message: "Rule acknowledged".to_string() }
                        }
                    }
                    IpcRequest::EmergencyRestore => {
                        engine_clone.emergency_restore();
                        IpcResponse::Ok { message: "Emergency restore completed".to_string() }
                    }
                    _ => IpcResponse::Ok { message: "Command processed".to_string() },
                }
            }
        };

        if let Ok(resp_frame) = encode_frame(&resp) {
            let _ = server.write_all(&resp_frame).await;
        }

        // Prepare pipe instance for subsequent connections
        server = create_named_pipe_server(false)?;
    }

    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Windows Service Control Manager (SCM) Management Implementations (Phase 2)
// ------------------------------------------------------------------------------------------------

fn install_service() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = format!("\"{}\"", exe_path.to_string_lossy());
    let exe_wide: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();

    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();
    let display_name_wide: Vec<u16> = "BandwidthX Enforcement Service".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm.is_null() {
            return Err(format!("Failed to open SCM (Run as Administrator): {}", std::io::Error::last_os_error()));
        }

        let service = CreateServiceW(
            scm,
            service_name_wide.as_ptr(),
            display_name_wide.as_ptr(),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_AUTO_START,
            SERVICE_ERROR_NORMAL,
            exe_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );

        let err = std::io::Error::last_os_error();

        if service.is_null() {
            CloseServiceHandle(scm);
            return Err(format!("Failed to create service: {}", err));
        }

        // Configure service failure recovery actions (Auto-restart after 5s and 10s)
        let mut actions = [
            SC_ACTION { Type: SC_ACTION_RESTART, Delay: 5000 },
            SC_ACTION { Type: SC_ACTION_RESTART, Delay: 10000 },
            SC_ACTION { Type: 0, Delay: 0 },
        ];
        let mut failure_actions: SERVICE_FAILURE_ACTIONSW = std::mem::zeroed();
        failure_actions.dwResetPeriod = 86400; // 24 hours
        failure_actions.cActions = actions.len() as u32;
        failure_actions.lpsaActions = actions.as_mut_ptr();

        let change_res = ChangeServiceConfig2W(
            service,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            &mut failure_actions as *mut _ as *mut c_void,
        );

        if change_res == 0 {
            let config_err = std::io::Error::last_os_error();
            DeleteService(service);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);
            return Err(format!("Failed to configure service failure recovery actions: {}", config_err));
        }

        // Query and authoritatively verify the configured recovery actions from SCM
        let mut bytes_needed: u32 = 0;
        let _ = QueryServiceConfig2W(
            service,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            std::ptr::null_mut(),
            0,
            &mut bytes_needed,
        );

        if bytes_needed == 0 {
            DeleteService(service);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);
            return Err("Failed to query configured failure actions buffer size from SCM".to_string());
        }

        let mut config_buf = vec![0u8; bytes_needed as usize];
        let query_res = QueryServiceConfig2W(
            service,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            config_buf.as_mut_ptr(),
            bytes_needed,
            &mut bytes_needed,
        );

        if query_res == 0 {
            let query_err = std::io::Error::last_os_error();
            DeleteService(service);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);
            return Err(format!("Failed to verify service recovery configuration: {}", query_err));
        }

        let verified_actions = &*(config_buf.as_ptr() as *const SERVICE_FAILURE_ACTIONSW);
        if verified_actions.cActions != actions.len() as u32 || verified_actions.lpsaActions.is_null() {
            DeleteService(service);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);
            return Err("Verification failed: SCM did not store the requested failure recovery actions".to_string());
        }

        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        println!("✔ BandwidthX Service installed successfully with auto-start and verified failure recovery configuration.");
        Ok(())
    }
}

fn uninstall_service() -> Result<(), String> {
    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm.is_null() {
            return Err(format!("Failed to open SCM (Run as Administrator): {}", std::io::Error::last_os_error()));
        }

        let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_STOP | DELETE);
        if service.is_null() {
            CloseServiceHandle(scm);
            return Err("BandwidthX Service is not installed or accessible".to_string());
        }

        let mut status: SERVICE_STATUS = std::mem::zeroed();
        let _ = ControlService(service, SERVICE_CONTROL_STOP, &mut status);

        let res = DeleteService(service);
        let err = std::io::Error::last_os_error();
        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if res != 0 {
            println!("✔ BandwidthX Service uninstalled successfully.");
            Ok(())
        } else {
            Err(format!("Failed to delete service: {}", err))
        }
    }
}

fn start_service() -> Result<(), String> {
    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm.is_null() {
            return Err(format!("Failed to open SCM: {}", std::io::Error::last_os_error()));
        }

        let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_ALL_ACCESS);
        if service.is_null() {
            CloseServiceHandle(scm);
            return Err("BandwidthX Service is not installed".to_string());
        }

        let res = StartServiceW(service, 0, std::ptr::null_mut());
        let err = std::io::Error::last_os_error();
        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if res != 0 {
            println!("✔ BandwidthX Service started.");
            Ok(())
        } else {
            Err(format!("Failed to start service: {}", err))
        }
    }
}

fn stop_service() -> Result<(), String> {
    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm.is_null() {
            return Err(format!("Failed to open SCM: {}", std::io::Error::last_os_error()));
        }

        let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_STOP);
        if service.is_null() {
            CloseServiceHandle(scm);
            return Err("BandwidthX Service is not installed".to_string());
        }

        let mut status: SERVICE_STATUS = std::mem::zeroed();
        let res = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
        let err = std::io::Error::last_os_error();
        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if res != 0 {
            println!("✔ BandwidthX Service stopped.");
            Ok(())
        } else {
            Err(format!("Failed to stop service: {}", err))
        }
    }
}

fn query_service_status() -> Result<(), String> {
    let service_name_wide: Vec<u16> = "BandwidthX".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm.is_null() {
            return Err(format!("Failed to open SCM: {}", std::io::Error::last_os_error()));
        }

        let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_ALL_ACCESS);
        if service.is_null() {
            CloseServiceHandle(scm);
            return Err("BandwidthX Service is not installed".to_string());
        }

        let mut ssp: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut bytes_needed = 0u32;
        let res = QueryServiceStatusEx(
            service,
            SC_STATUS_PROCESS_INFO,
            &mut ssp as *mut _ as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut bytes_needed,
        );

        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if res != 0 {
            let state_str = match ssp.dwCurrentState {
                SERVICE_RUNNING => "RUNNING",
                SERVICE_STOPPED => "STOPPED",
                SERVICE_START_PENDING => "START_PENDING",
                SERVICE_STOP_PENDING => "STOP_PENDING",
                _ => "UNKNOWN",
            };
            println!("Service Status: {} (PID: {})", state_str, ssp.dwProcessId);
            Ok(())
        } else {
            Err("Failed to query service status".to_string())
        }
    }
}

/// Runs service in interactive console mode (used when invoked from terminal or for debugging)
fn run_console_mode() -> Result<(), Box<dyn std::error::Error>> {
    println!("======================================================");
    println!("  BandwidthX Authoritative Privileged Service v2.3");
    println!("  Mode: Interactive Console (Non-SCM)");
    println!("  IPC Address: {}", BANDWIDTHX_IPC_PIPE_NAME);
    println!("  DACL: Verified Builtin Administrators + LocalSystem");
    println!("======================================================");

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async {
        let (shutdown_send, shutdown_recv) = tokio::sync::broadcast::channel::<()>(1);
        *SHUTDOWN_TX.lock() = Some(shutdown_send);

        let engine = Arc::new(HardenedWfpEngine::new());
        *ACTIVE_ENGINE.write() = Some(Arc::clone(&engine));

        println!("✔ Listening for BandwidthX desktop connections on secured Named Pipe...");
        run_ipc_server(engine, shutdown_recv).await
    })
}

// ------------------------------------------------------------------------------------------------
// Main Entrypoint
// ------------------------------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                if let Err(e) = install_service() {
                    eprintln!("Error installing service: {}", e);
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--uninstall" => {
                if let Err(e) = uninstall_service() {
                    eprintln!("Error uninstalling service: {}", e);
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--start" => {
                if let Err(e) = start_service() {
                    eprintln!("Error starting service: {}", e);
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--stop" => {
                if let Err(e) = stop_service() {
                    eprintln!("Error stopping service: {}", e);
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--status" => {
                if let Err(e) = query_service_status() {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--console" => {
                return run_console_mode();
            }
            "--version" => {
                println!("BandwidthX Privileged Network Service v2.3.0");
                return Ok(());
            }
            "--help" => {
                println!("BandwidthX Privileged Network Service");
                println!("Usage: bandwidthx-service.exe [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --install     Register service with Windows SCM (Auto-start & Verified Recovery)");
                println!("  --uninstall   Unregister service from Windows SCM");
                println!("  --start       Start the registered service via SCM");
                println!("  --stop        Stop the registered service via SCM");
                println!("  --status      Query service running status from SCM");
                println!("  --console     Run in interactive console mode for debugging");
                println!("  --version     Display version information");
                return Ok(());
            }
            _ => {}
        }
    }

    // When launched without arguments, attempt to connect to Windows Service Control Manager (SCM)
    let service_name_wide: Vec<u16> = "BandwidthX\0".encode_utf16().collect();
    let service_table: [SERVICE_TABLE_ENTRYW; 2] = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: service_name_wide.as_ptr() as *mut u16,
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: std::ptr::null_mut(),
            lpServiceProc: None,
        },
    ];

    let scm_res = unsafe { StartServiceCtrlDispatcherW(service_table.as_ptr()) };
    if scm_res == 0 {
        let err = unsafe { GetLastError() };
        // ERROR_FAILED_SERVICE_CONTROLLER_CONNECT (1063) indicates the process was launched directly
        // from a console/terminal rather than by the Windows Service Control Manager.
        if err == 1063 {
            return run_console_mode();
        } else {
            eprintln!("StartServiceCtrlDispatcherW failed with error code: {}", err);
            std::process::exit(3);
        }
    }

    Ok(())
}
