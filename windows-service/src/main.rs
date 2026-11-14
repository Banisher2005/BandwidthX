use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, FILETIME, LocalFree, GetLastError};
use windows_sys::Win32::Security::{
    Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY, SECURITY_ATTRIBUTES,
    CheckTokenMembership, AllocateAndInitializeSid, FreeSid, RevertToSelf,
    GetKernelObjectSecurity, DACL_SECURITY_INFORMATION,
    SECURITY_NT_AUTHORITY,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, ImpersonateNamedPipeClient};
use windows_sys::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, GetProcessTimes, WaitForSingleObject,
};
use windows_sys::Win32::System::Services::{
    OpenSCManagerW, CreateServiceW, OpenServiceW, DeleteService, CloseServiceHandle,
    StartServiceW, ControlService, QueryServiceStatusEx,
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

use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus
};

use std::sync::atomic::{AtomicIsize, Ordering};

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

/// Handler callback for Windows SCM control requests
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

            // 1. Notify Tokio server loop to stop cleanly
            if let Some(tx) = SHUTDOWN_TX.lock().as_ref() {
                let _ = tx.send(());
            }

            // 2. Perform emergency WFP policy restoration
            if let Some(engine) = ACTIVE_ENGINE.read().as_ref() {
                eprintln!("[SCM] Controlled shutdown: performing emergency WFP filter purge...");
                engine.emergency_restore();
            }

            0 // NO_ERROR
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
// Fail-Closed Named Pipe Security & Client Authorization
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

/// Verifies that the created named pipe handle has an effective, non-null security descriptor
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

        Ok(())
    }
}

/// Instantiates the secured Named Pipe server with explicit Administrator/LocalSystem DACL.
/// Fails closed if SDDL conversion or secure creation fails (NO unsecured fallback).
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

/// Comprehensive client authentication & authorization.
/// Validates both:
/// 1. Windows Security Reference Monitor check via `ImpersonateNamedPipeClient` & `CheckTokenMembership` (BUILTIN\Administrators)
/// 2. Client process token elevation check (`TokenElevation`)
fn authorize_client(pipe_handle: HANDLE) -> Result<u32, String> {
    unsafe {
        // 1. Get client PID
        let mut client_pid: u32 = 0;
        if GetNamedPipeClientProcessId(pipe_handle, &mut client_pid) == 0 {
            return Err("Unable to query named pipe client PID".to_string());
        }

        // 2. Impersonate client to leverage Windows Security Reference Monitor
        if ImpersonateNamedPipeClient(pipe_handle) == 0 {
            return Err("Failed to impersonate named pipe client for authorization check".to_string());
        }

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

        // Revert impersonation back to service context
        let _ = RevertToSelf();

        if is_admin == 0 {
            return Err("Client identity is not an authorized Administrator or LocalSystem".to_string());
        }

        // 4. Additionally verify client process token elevation
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, client_pid);
        if handle.is_null() {
            return Err(format!("Client PID {} inaccessible", client_pid));
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
// Windows Service Control Manager (SCM) Lifecycle & ServiceMain
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
        if let Err(e) = run_ipc_server(engine, shutdown_recv).await {
            eprintln!("[Service] IPC server loop encountered error: {}", e);
        }

        // Report SERVICE_STOP_PENDING during graceful teardown
        update_service_status(SERVICE_STOP_PENDING, 0, 2, 5000);
    });

    // 4. Report SERVICE_STOPPED
    update_service_status(SERVICE_STOPPED, 0, 0, 0);
    SERVICE_STATUS_HANDLE_RAW.store(0, Ordering::SeqCst);
}

/// Asynchronous IPC server loop
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

        // Authenticate & authorize client process before processing commands
        let raw_handle = server.as_raw_handle() as HANDLE;
        let client_auth = authorize_client(raw_handle);

        let mut len_buf = [0u8; 4];
        if server.read_exact(&mut len_buf).await.is_ok() {
            let frame_len = u32::from_be_bytes(len_buf) as usize;
            if frame_len <= 1024 * 1024 {
                let mut payload_buf = vec![0u8; frame_len];
                if server.read_exact(&mut payload_buf).await.is_ok() {
                    let mut full_frame = Vec::with_capacity(4 + frame_len);
                    full_frame.extend_from_slice(&len_buf);
                    full_frame.extend_from_slice(&payload_buf);

                    let resp: IpcResponse = match client_auth {
                        Err(auth_err) => {
                            eprintln!("⚠ Unauthorized IPC connection rejected: {}", auth_err);
                            IpcResponse::Error {
                                code: 403,
                                message: format!("Access Denied: {}", auth_err),
                            }
                        }
                        Ok(_client_pid) => {
                            match decode_frame::<IpcRequest>(&full_frame) {
                                Ok(req) => match req {
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
                                            version: "2.2.0".to_string(),
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
                                },
                                Err(e) => IpcResponse::Error {
                                    code: 400,
                                    message: format!("Malformed IPC frame: {}", e),
                                },
                            }
                        }
                    };

                    if let Ok(resp_frame) = encode_frame(&resp) {
                        let _ = server.write_all(&resp_frame).await;
                    }
                }
            }
        }

        // Prepare pipe instance for subsequent connections
        server = create_named_pipe_server(false)?;
    }

    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Windows Service Control Manager (SCM) Management Implementations
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

        let _ = ChangeServiceConfig2W(
            service,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            &mut failure_actions as *mut _ as *mut c_void,
        );

        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        println!("✔ BandwidthX Service installed successfully with auto-start and failure recovery configuration.");
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
    println!("  BandwidthX Authoritative Privileged Service v2.2");
    println!("  Mode: Interactive Console (Non-SCM)");
    println!("  IPC Address: {}", BANDWIDTHX_IPC_PIPE_NAME);
    println!("  DACL: Explicit Builtin Administrators + LocalSystem");
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
                println!("BandwidthX Privileged Network Service v2.2.0");
                return Ok(());
            }
            "--help" => {
                println!("BandwidthX Privileged Network Service");
                println!("Usage: bandwidthx-service.exe [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --install     Register service with Windows SCM (Auto-start & Recovery)");
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
            return Err(format!("SCM Dispatcher error: {}", err).into());
        }
    }

    Ok(())
}
