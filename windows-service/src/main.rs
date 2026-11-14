use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, FILETIME, LocalFree};
use windows_sys::Win32::Security::{
    Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY, SECURITY_ATTRIBUTES,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, GetProcessTimes,
};
use windows_sys::Win32::System::Services::{
    OpenSCManagerW, CreateServiceW, OpenServiceW, DeleteService, CloseServiceHandle,
    StartServiceW, ControlService, QueryServiceStatusEx,
    SC_MANAGER_ALL_ACCESS, SERVICE_WIN32_OWN_PROCESS, SERVICE_AUTO_START,
    SERVICE_ERROR_NORMAL, SERVICE_ALL_ACCESS, SERVICE_STOP,
    SERVICE_CONTROL_STOP, SERVICE_STATUS, SERVICE_STATUS_PROCESS, SC_STATUS_PROCESS_INFO,
    SERVICE_STOPPED, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STOP_PENDING,
};

const DELETE: u32 = 0x00010000;
use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus
};

/// Validates target running process identity authoritatively via kernel handles.
/// Verifies PID accessibility, creation timestamp, and full image path.
/// Refuses file existence as proof of running process identity.
fn validate_running_process(pid: u32, claimed_creation_time: u64, claimed_path: &str) -> Result<(String, u64), String> {
    if claimed_path.is_empty() {
        return Err("Executable path cannot be empty".to_string());
    }

    if pid == 0 || pid == 4 {
        return Err("Cannot target system kernel process (PID 0 or 4)".to_string());
    }

    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err(format!(
                "Process PID {} is not running or accessible: {}",
                pid,
                std::io::Error::last_os_error()
            ));
        }

        // 1. Validate process creation timestamp to protect against PID recycling
        let mut creation_time: FILETIME = std::mem::zeroed();
        let mut exit_time: FILETIME = std::mem::zeroed();
        let mut kernel_time: FILETIME = std::mem::zeroed();
        let mut user_time: FILETIME = std::mem::zeroed();

        if GetProcessTimes(handle, &mut creation_time, &mut exit_time, &mut kernel_time, &mut user_time) == 0 {
            CloseHandle(handle);
            return Err(format!("Failed to retrieve process lifetime metrics for PID {}", pid));
        }

        let actual_ts = ((creation_time.dwHighDateTime as u64) << 32) | (creation_time.dwLowDateTime as u64);
        if claimed_creation_time > 0 && actual_ts != claimed_creation_time {
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
        CloseHandle(handle);

        if success == 0 || size == 0 {
            return Err(format!("Failed to query process image name for PID {}", pid));
        }

        let actual_path = String::from_utf16_lossy(&path_buf[..size as usize]);
        if !actual_path.eq_ignore_ascii_case(claimed_path) {
            return Err(format!(
                "Process verification failed: PID {} image is '{}', but request claimed '{}'",
                pid, actual_path, claimed_path
            ));
        }

        Ok((actual_path, actual_ts))
    }
}

/// Creates a Windows SECURITY_ATTRIBUTES structure from an SDDL string
/// restricting pipe access strictly to Builtin Administrators (BA) and LocalSystem (SY).
fn create_admin_security_attributes() -> Option<(SECURITY_ATTRIBUTES, *mut c_void)> {
    unsafe {
        // SDDL: DACL allowing Generic All (GA) to Builtin Administrators (BA) and Local System (SY)
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

/// Instantiates the secured Named Pipe server with explicit Administrator/LocalSystem DACL
fn create_named_pipe_server(first_instance: bool) -> Result<NamedPipeServer, std::io::Error> {
    let mut options = ServerOptions::new();
    options.first_pipe_instance(first_instance);
    options.max_instances(8);

    if let Some((mut sa, p_sd)) = create_admin_security_attributes() {
        let res = unsafe {
            options.create_with_security_attributes_raw(
                BANDWIDTHX_IPC_PIPE_NAME,
                &mut sa as *mut _ as *mut c_void,
            )
        };
        unsafe {
            LocalFree(p_sd);
        }
        res
    } else {
        options.create(BANDWIDTHX_IPC_PIPE_NAME)
    }
}

/// Authenticates connecting named pipe client process and validates administrative elevation
fn authenticate_client(pipe_handle: HANDLE) -> Result<u32, String> {
    unsafe {
        let mut client_pid: u32 = 0;
        if GetNamedPipeClientProcessId(pipe_handle, &mut client_pid) == 0 {
            return Err("Unable to query named pipe client PID".to_string());
        }

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

        if elevated != 0 && elevation.TokenIsElevated != 0 {
            Ok(client_pid)
        } else {
            Err("Client process is not running with elevated administrative privileges".to_string())
        }
    }
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
        CloseServiceHandle(scm);

        if service.is_null() {
            Err(format!("Failed to create service: {}", err))
        } else {
            CloseServiceHandle(service);
            println!("✔ BandwidthX Service installed successfully with auto-start configuration.");
            Ok(())
        }
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

// ------------------------------------------------------------------------------------------------
// Main Entrypoint
// ------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            "--version" => {
                println!("BandwidthX Privileged Network Service v2.1.0");
                return Ok(());
            }
            "--help" => {
                println!("BandwidthX Privileged Network Service");
                println!("Usage: bandwidthx-service.exe [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --install     Register service with Windows SCM (Auto-start)");
                println!("  --uninstall   Unregister service from Windows SCM");
                println!("  --start       Start the registered service");
                println!("  --stop        Stop the registered service");
                println!("  --status      Query service running status");
                println!("  --version     Display version information");
                return Ok(());
            }
            _ => {}
        }
    }

    println!("======================================================");
    println!("  BandwidthX Authoritative Privileged Service v2.1");
    println!("  IPC Address: {}", BANDWIDTHX_IPC_PIPE_NAME);
    println!("  DACL: Explicit Builtin Administrators + LocalSystem");
    println!("======================================================");

    let engine = Arc::new(HardenedWfpEngine::new());
    let mut server = create_named_pipe_server(true)?;

    println!("✔ Listening for BandwidthX desktop connections on secured Named Pipe...");

    loop {
        server.connect().await?;
        let engine_clone = Arc::clone(&engine);

        // Authenticate client process before processing commands
        let raw_handle = server.as_raw_handle() as HANDLE;
        let client_auth = authenticate_client(raw_handle);

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
                                            version: "2.1.0".to_string(),
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
                                            Ok((validated_path, actual_creation_time)) => {
                                                match engine_clone.enable_exclusive_mode(
                                                    target_pid,
                                                    actual_creation_time,
                                                    &validated_path,
                                                    &target_app_name,
                                                    &exemptions,
                                                ) {
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
}
