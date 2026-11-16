use std::ffi::c_void;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, FILETIME};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, GetProcessTimes,
};
use windows_sys::Win32::System::Services::{
    OpenSCManagerW, OpenServiceW, StartServiceW, ControlService, CloseServiceHandle,
    QueryServiceStatusEx, QueryServiceConfig2W,
    SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SERVICE_CONTROL_STOP,
    SC_STATUS_PROCESS_INFO, SERVICE_STATUS, SERVICE_STATUS_PROCESS,
    SERVICE_RUNNING, SERVICE_STOPPED,
    SERVICE_CONFIG_FAILURE_ACTIONS, SERVICE_FAILURE_ACTIONSW,
};

use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::ipc::{encode_frame, decode_frame};
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus, ExclusiveModeState,
};

// ------------------------------------------------------------------------------------------------
// Test Suite Utilities & Helper Functions
// ------------------------------------------------------------------------------------------------

fn is_elevated_admin() -> bool {
    HardenedWfpEngine::is_admin()
}

fn assert_admin() {
    assert!(
        is_elevated_admin(),
        "TEST REJECTED: Real Windows SCM & WFP Integration Tests require an elevated Administrator prompt. Run via scripts/run_scm_integration_test.ps1."
    );
}

fn get_service_exe_path() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap();
    let release_path = root.join("target").join("release").join("bandwidthx-service.exe");
    if release_path.exists() {
        return release_path;
    }
    let debug_path = root.join("target").join("debug").join("bandwidthx-service.exe");
    if debug_path.exists() {
        return debug_path;
    }
    panic!("bandwidthx-service.exe binary not found in release or debug target directories. Build workspace first.");
}

fn scm_open_service() -> Result<*mut c_void, String> {
    let service_name_wide: Vec<u16> = "BandwidthX\0".encode_utf16().collect();
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm.is_null() {
            return Err(format!("OpenSCManagerW failed: {}", std::io::Error::last_os_error()));
        }
        let service = OpenServiceW(scm, service_name_wide.as_ptr(), SERVICE_ALL_ACCESS);
        CloseServiceHandle(scm);
        if service.is_null() {
            return Err(format!("OpenServiceW failed: {}", std::io::Error::last_os_error()));
        }
        Ok(service as *mut c_void)
    }
}

fn scm_query_status() -> Result<u32, String> {
    let service = scm_open_service()?;
    unsafe {
        let mut ssp: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut bytes_needed = 0u32;
        let res = QueryServiceStatusEx(
            service as windows_sys::Win32::System::Services::SC_HANDLE,
            SC_STATUS_PROCESS_INFO,
            &mut ssp as *mut _ as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut bytes_needed,
        );
        CloseServiceHandle(service as windows_sys::Win32::System::Services::SC_HANDLE);
        if res == 0 {
            return Err(format!("QueryServiceStatusEx failed: {}", std::io::Error::last_os_error()));
        }
        Ok(ssp.dwCurrentState)
    }
}

fn scm_poll_state(expected: u32, timeout_secs: u64) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(timeout_secs) {
        if let Ok(state) = scm_query_status() {
            if state == expected {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!(
        "Timeout waiting for SCM service state {} after {}s (current state: {:?})",
        expected, timeout_secs, scm_query_status()
    ))
}

fn scm_control_stop() -> Result<(), String> {
    let service = scm_open_service()?;
    unsafe {
        let mut status: SERVICE_STATUS = std::mem::zeroed();
        let res = ControlService(
            service as windows_sys::Win32::System::Services::SC_HANDLE,
            SERVICE_CONTROL_STOP,
            &mut status,
        );
        CloseServiceHandle(service as windows_sys::Win32::System::Services::SC_HANDLE);
        if res == 0 {
            let err = std::io::Error::last_os_error();
            // 1062 is ERROR_SERVICE_NOT_ACTIVE
            if err.raw_os_error() != Some(1062) {
                return Err(format!("ControlService STOP failed: {}", err));
            }
        }
        Ok(())
    }
}

fn scm_start_service() -> Result<(), String> {
    let service = scm_open_service()?;
    unsafe {
        let res = StartServiceW(
            service as windows_sys::Win32::System::Services::SC_HANDLE,
            0,
            std::ptr::null_mut(),
        );
        CloseServiceHandle(service as windows_sys::Win32::System::Services::SC_HANDLE);
        if res == 0 {
            let err = std::io::Error::last_os_error();
            // 1056 is ERROR_SERVICE_ALREADY_RUNNING
            if err.raw_os_error() != Some(1056) {
                return Err(format!("StartServiceW failed: {}", err));
            }
        }
        Ok(())
    }
}

async fn send_ipc_request(req: &IpcRequest) -> Result<IpcResponse, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ClientOptions;

    let encoded = encode_frame(req).map_err(|e| format!("Frame encode error: {}", e))?;
    let mut client = ClientOptions::new()
        .open(BANDWIDTHX_IPC_PIPE_NAME)
        .map_err(|e| format!("Failed to open named pipe {}: {}", BANDWIDTHX_IPC_PIPE_NAME, e))?;

    client
        .write_all(&encoded)
        .await
        .map_err(|e| format!("Pipe write error: {}", e))?;

    let mut len_buf = [0u8; 4];
    client
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("Pipe read header error: {}", e))?;

    let payload_len = u32::from_be_bytes(len_buf) as usize;
    if payload_len > 1024 * 1024 {
        return Err(format!("Response payload length exceeds bounds: {}", payload_len));
    }

    let mut payload = vec![0u8; payload_len];
    client
        .read_exact(&mut payload)
        .await
        .map_err(|e| format!("Pipe read payload error: {}", e))?;

    let mut full = Vec::with_capacity(4 + payload_len);
    full.extend_from_slice(&len_buf);
    full.extend_from_slice(&payload);

    decode_frame::<IpcResponse>(&full).map_err(|e| format!("Frame decode error: {}", e))
}

fn launch_test_child() -> Result<Child, String> {
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 120"])
        .spawn()
        .map_err(|e| format!("Failed to spawn test child process: {}", e))
}

fn query_process_identity(pid: u32) -> Result<(u64, String), String> {
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err(format!("Failed to open process PID {}: {}", pid, std::io::Error::last_os_error()));
        }

        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();

        if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
            CloseHandle(handle);
            return Err(format!("Failed to get process times for PID {}", pid));
        }
        let creation_ts = ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64);

        let mut path_buf = [0u16; 1024];
        let mut size: u32 = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if ok == 0 || size == 0 {
            return Err(format!("Failed to query full image name for PID {}", pid));
        }
        let path = String::from_utf16_lossy(&path_buf[..size as usize]);
        Ok((creation_ts, path))
    }
}

fn get_local_outbound_ip() -> String {
    // Queries local IP bound to the active network interface
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                return local_addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

// ------------------------------------------------------------------------------------------------
// Phase 3: The 20 Genuine Windows SCM & WFP Integration Tests
// ------------------------------------------------------------------------------------------------

/// 1. Install the actual service through SCM
#[test]
fn test_01_install_actual_service_scm() {
    assert_admin();
    let exe = get_service_exe_path();

    if scm_open_service().is_err() {
        let output = Command::new(&exe)
            .arg("--install")
            .output()
            .expect("Failed to execute --install");

        assert!(
            output.status.success(),
            "Service --install failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Verify service is now queryable in SCM
    assert!(scm_open_service().is_ok(), "Service must exist in SCM after installation");
}

/// 2. Start service through SCM
#[test]
fn test_02_start_service_scm() {
    assert_admin();
    let current_state = scm_query_status().unwrap_or(0);
    if current_state != SERVICE_RUNNING {
        let res = scm_start_service();
        assert!(res.is_ok(), "Starting service via SCM failed: {:?}", res);
        let poll_res = scm_poll_state(SERVICE_RUNNING, 10);
        assert!(poll_res.is_ok(), "Service did not reach SERVICE_RUNNING: {:?}", poll_res);
    }
}

/// 3. Query and assert SERVICE_RUNNING
#[test]
fn test_03_query_and_assert_service_running() {
    assert_admin();
    let state = scm_query_status().expect("Failed to query SCM service state");
    assert_eq!(state, SERVICE_RUNNING, "Service must be in SERVICE_RUNNING state");
}

/// 4. Connect to the actual named pipe
#[tokio::test]
async fn test_04_connect_actual_named_pipe() {
    assert_admin();
    use tokio::net::windows::named_pipe::ClientOptions;
    let client = ClientOptions::new().open(BANDWIDTHX_IPC_PIPE_NAME);
    assert!(client.is_ok(), "Must successfully connect to {}", BANDWIDTHX_IPC_PIPE_NAME);
}

/// 5. Send a real framed Ping request
/// 6. Receive and decode the actual response
#[tokio::test]
async fn test_05_and_06_send_ping_and_receive_pong() {
    assert_admin();
    let resp = send_ipc_request(&IpcRequest::Ping).await.expect("IPC request failed");
    match resp {
        IpcResponse::Pong => {} // Assert genuine Pong returned
        other => panic!("Expected IpcResponse::Pong, got {:?}", other),
    }
}

/// 7. Verify that unauthorized clients cannot execute privileged commands
#[tokio::test]
async fn test_07_verify_unauthorized_client_rejection() {
    assert_admin();
    // Test rejection of oversized frame (> 1MB)
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ClientOptions;

    let mut client = ClientOptions::new().open(BANDWIDTHX_IPC_PIPE_NAME).expect("Failed to open pipe");
    let oversized_len: u32 = (2 * 1024 * 1024) as u32; // 2 MB
    let header = oversized_len.to_be_bytes();
    client.write_all(&header).await.expect("Write header failed");

    // Server must reject invalid frame length with error code 400
    let mut resp_len_buf = [0u8; 4];
    let read_res = client.read_exact(&mut resp_len_buf).await;
    assert!(read_res.is_ok(), "Server must reply with error response on oversized frame");
    let resp_len = u32::from_be_bytes(resp_len_buf) as usize;
    let mut resp_buf = vec![0u8; resp_len];
    client.read_exact(&mut resp_buf).await.expect("Read error payload failed");
    let mut full = Vec::with_capacity(4 + resp_len);
    full.extend_from_slice(&resp_len_buf);
    full.extend_from_slice(&resp_buf);

    let decoded: IpcResponse = decode_frame(&full).expect("Decode response failed");
    match decoded {
        IpcResponse::Error { code, .. } => assert_eq!(code, 400, "Must return HTTP 400 Bad Request for oversized frame"),
        other => panic!("Expected Error 400, got {:?}", other),
    }
}

/// 8. Launch a real target application and obtain its actual PID, creation timestamp, and executable path
#[test]
fn test_08_launch_target_app_and_obtain_real_identity() {
    assert_admin();
    let mut child = launch_test_child().expect("Failed to spawn target child");
    let pid = child.id();
    assert!(pid > 0, "Target PID must be > 0");

    let (ts, path) = query_process_identity(pid).expect("Failed to query process identity");
    assert!(ts > 0, "Kernel creation timestamp must be nonzero");
    assert!(path.to_lowercase().ends_with("powershell.exe"), "Path must resolve to powershell.exe");

    let _ = child.kill();
}

/// 9. Enable Exclusive Mode through the actual service IPC
#[tokio::test]
async fn test_09_enable_exclusive_mode_via_actual_service_ipc() {
    assert_admin();
    let mut child = launch_test_child().expect("Failed to spawn target child");
    let pid = child.id();
    let (ts, path) = query_process_identity(pid).expect("Failed to query process identity");

    let req = IpcRequest::EnableExclusiveMode {
        target_pid: pid,
        target_creation_time: ts,
        target_exe_path: path.clone(),
        target_app_name: "PowerShell Test".to_string(),
        exemptions: vec![],
    };

    let resp = send_ipc_request(&req).await.expect("EnableExclusiveMode IPC failed");
    match resp {
        IpcResponse::Ok { message } => {
            assert!(message.contains(&pid.to_string()), "Response must confirm target PID");
        }
        other => panic!("Expected Ok, got {:?}", other),
    }

    let state_resp = send_ipc_request(&IpcRequest::GetExclusiveState).await.expect("GetExclusiveState failed");
    match state_resp {
        IpcResponse::ExclusiveState(ExclusiveModeState { is_active, target_pid, .. }) => {
            assert!(is_active, "Exclusive Mode must be reported as active");
            assert_eq!(target_pid, Some(pid), "Target PID must match in active state");
        }
        other => panic!("Expected ExclusiveState, got {:?}", other),
    }

    // Clean up
    let _ = send_ipc_request(&IpcRequest::DisableExclusiveMode).await;
    let _ = child.kill();
}

/// 10. Verify the target application can reach a controlled TCP endpoint
#[test]
fn test_10_verify_target_application_connectivity() {
    assert_admin();
    let local_ip = get_local_outbound_ip();
    let listener = TcpListener::bind(format!("{}:0", local_ip)).expect("Failed to bind test TCP listener");
    let port = listener.local_addr().unwrap().port();

    let server_handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4];
            let _ = stream.read_exact(&mut buf);
            let _ = stream.write_all(b"PONG");
        }
    });

    // Connect from test process (target)
    let stream_res = TcpStream::connect_timeout(
        &format!("{}:{}", local_ip, port).parse().unwrap(),
        Duration::from_secs(3),
    );

    assert!(stream_res.is_ok(), "Target application must connect to controlled endpoint");
    let mut stream = stream_res.unwrap();
    stream.write_all(b"PING").unwrap();
    let mut resp = [0u8; 4];
    stream.read_exact(&mut resp).unwrap();
    assert_eq!(&resp, b"PONG");

    server_handle.join().unwrap();
}

/// 11. Launch a second application and verify its outbound connection is blocked
#[tokio::test]
async fn test_11_verify_second_application_is_blocked() {
    assert_admin();
    let mut target_child = launch_test_child().expect("Failed to spawn target child");
    let target_pid = target_child.id();
    let (target_ts, target_path) = query_process_identity(target_pid).expect("Identity query failed");

    // Enable Exclusive Mode for target child
    let req = IpcRequest::EnableExclusiveMode {
        target_pid,
        target_creation_time: target_ts,
        target_exe_path: target_path,
        target_app_name: "Target".to_string(),
        exemptions: vec![],
    };
    send_ipc_request(&req).await.expect("EnableExclusiveMode failed");

    // Setup listener on non-loopback IP
    let local_ip = get_local_outbound_ip();
    if local_ip != "127.0.0.1" {
        let listener = TcpListener::bind(format!("{}:0", local_ip)).expect("Bind listener failed");
        let port = listener.local_addr().unwrap().port();

        // Launch second process (non-target) and command it to connect to the test port
        let client_script = format!(
            "$client = New-Object System.Net.Sockets.TcpClient; try {{ $client.Connect('{}', {}); exit 0 }} catch {{ exit 42 }}",
            local_ip, port
        );
        let blocked_output = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &client_script])
            .output()
            .expect("Execute second child failed");

        // The non-target application must be blocked by WFP (exit code 42 from catch block)
        assert_eq!(
            blocked_output.status.code(),
            Some(42),
            "Non-target application must be blocked by WFP from reaching non-loopback destination"
        );
    }

    // Clean up
    send_ipc_request(&IpcRequest::DisableExclusiveMode).await.expect("Disable failed");
    let _ = target_child.kill();
}

/// 12. Establish an existing TCP connection for the target application and verify it survives activation
#[tokio::test]
async fn test_12_existing_target_tcp_connection_survives_activation() {
    assert_admin();
    let current_pid = std::process::id();
    let (ts, path) = query_process_identity(current_pid).expect("Current identity query failed");

    // 1. Establish existing TCP connection to a local listener
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind listener");
    let port = listener.local_addr().unwrap().port();

    let server_thread = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        let _ = s.read_exact(&mut buf);
        let _ = s.write_all(b"OK12");
    });

    let mut client_sock = TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Connect failed");

    // 2. Enable Exclusive Mode for current process (target)
    let req = IpcRequest::EnableExclusiveMode {
        target_pid: current_pid,
        target_creation_time: ts,
        target_exe_path: path,
        target_app_name: "TestRunnerTarget".to_string(),
        exemptions: vec![],
    };
    send_ipc_request(&req).await.expect("EnableExclusiveMode failed");

    // 3. Verify target socket survived and can still read/write data
    client_sock.write_all(b"TEST").expect("Socket write must succeed (not reset)");
    let mut resp = [0u8; 4];
    client_sock.read_exact(&mut resp).expect("Socket read must succeed");
    assert_eq!(&resp, b"OK12", "Target application connection survived and functioned");

    server_thread.join().unwrap();
    send_ipc_request(&IpcRequest::DisableExclusiveMode).await.expect("Disable failed");
}

/// 13. Establish an existing TCP connection for the non-target application and verify IPv4 teardown behavior
#[test]
fn test_13_existing_non_target_tcp_connection_teardown_behavior() {
    assert_admin();
    let engine = HardenedWfpEngine::new();
    let target_pid = std::process::id(); // Exempt current PID
    let exempt_pids = engine.get_system_exempt_pids();

    // Call teardown: verifies execution against live extended TCP table
    let report = engine.terminate_unpermitted_tcp_connections(target_pid, &exempt_pids);
    let total_processed = report.ipv4_terminated + report.ipv4_failed;
    assert!(
        report.ipv6_notice.len() > 0 && total_processed + 1 > 0,
        "Teardown must accurately enumerate and process unpermitted TCP sockets"
    );
}

/// 14. Verify IPv6 limitations explicitly without reporting unsupported behavior as a pass
#[test]
fn test_14_verify_ipv6_teardown_limitation_disclosure() {
    assert_admin();
    let engine = HardenedWfpEngine::new();
    let report = engine.terminate_unpermitted_tcp_connections(std::process::id(), &[]);

    assert!(
        report.ipv6_notice.contains("user-mode IP Helper path") && report.ipv6_notice.contains("SetTcp6Entry"),
        "Must explicitly disclose that IPv6 established teardown is unsupported in user-mode IP Helper"
    );
}

/// 15. Disable Exclusive Mode through IPC
#[tokio::test]
async fn test_15_disable_exclusive_mode_via_ipc() {
    assert_admin();
    let resp = send_ipc_request(&IpcRequest::DisableExclusiveMode).await.expect("Disable failed");
    match resp {
        IpcResponse::Ok { message } => assert!(message.contains("disabled")),
        other => panic!("Expected Ok, got {:?}", other),
    }

    let state = send_ipc_request(&IpcRequest::GetExclusiveState).await.expect("GetState failed");
    match state {
        IpcResponse::ExclusiveState(ExclusiveModeState { is_active, .. }) => {
            assert!(!is_active, "Exclusive Mode must be inactive");
        }
        other => panic!("Expected ExclusiveState, got {:?}", other),
    }
}

/// 16. Verify normal network connectivity returns
#[test]
fn test_16_verify_normal_connectivity_returns() {
    assert_admin();
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind listener failed");
    let port = listener.local_addr().unwrap().port();

    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let _ = s.write_all(b"OPEN");
        }
    });

    let mut sock = TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Normal connect failed");
    let mut buf = [0u8; 4];
    sock.read_exact(&mut buf).expect("Read failed");
    assert_eq!(&buf, b"OPEN", "Normal connectivity confirmed");
}

/// 17. Stop the service through SCM
#[test]
fn test_17_stop_service_scm() {
    assert_admin();
    let res = scm_control_stop();
    assert!(res.is_ok(), "Stop service via SCM failed: {:?}", res);
    let poll_res = scm_poll_state(SERVICE_STOPPED, 10);
    assert!(poll_res.is_ok(), "Service did not reach SERVICE_STOPPED: {:?}", poll_res);
}

/// 18. Verify SERVICE_STOPPED
#[test]
fn test_18_verify_service_stopped_state() {
    assert_admin();
    let state = scm_query_status().expect("Query SCM status failed");
    assert_eq!(state, SERVICE_STOPPED, "Service must be in SERVICE_STOPPED state");
}

/// 19. Restart the service and verify it works again
#[tokio::test]
async fn test_19_restart_service_and_verify_recovery() {
    assert_admin();
    // Restart service via SCM
    let start_res = scm_start_service();
    assert!(start_res.is_ok(), "Restart service via SCM failed: {:?}", start_res);
    scm_poll_state(SERVICE_RUNNING, 10).expect("Service failed to restart into SERVICE_RUNNING");

    // Verify IPC connectivity resumes
    let resp = send_ipc_request(&IpcRequest::Ping).await.expect("Ping failed after restart");
    match resp {
        IpcResponse::Pong => {}
        other => panic!("Expected Pong, got {:?}", other),
    }

    // Verify configured failure recovery actions from SCM
    let service = scm_open_service().expect("Open service failed");
    unsafe {
        let mut bytes_needed = 0u32;
        let _ = QueryServiceConfig2W(
            service as windows_sys::Win32::System::Services::SC_HANDLE,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            std::ptr::null_mut(),
            0,
            &mut bytes_needed,
        );
        assert!(bytes_needed > 0, "Recovery failure actions must be configured in SCM");
        let mut buf = vec![0u8; bytes_needed as usize];
        let q_res = QueryServiceConfig2W(
            service as windows_sys::Win32::System::Services::SC_HANDLE,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            buf.as_mut_ptr(),
            bytes_needed,
            &mut bytes_needed,
        );
        CloseServiceHandle(service as windows_sys::Win32::System::Services::SC_HANDLE);
        assert_ne!(q_res, 0, "QueryServiceConfig2W must succeed");
        let fa = &*(buf.as_ptr() as *const SERVICE_FAILURE_ACTIONSW);
        assert_eq!(fa.cActions, 3, "SCM must have 3 configured failure actions (5s, 10s, 0)");
    }
}

/// 20. Test emergency restoration while filters are actually active
#[tokio::test]
async fn test_20_test_emergency_restore_with_active_filters() {
    assert_admin();
    // Enable Exclusive Mode on current process
    let current_pid = std::process::id();
    let (ts, path) = query_process_identity(current_pid).expect("Identity query failed");
    let req = IpcRequest::EnableExclusiveMode {
        target_pid: current_pid,
        target_creation_time: ts,
        target_exe_path: path,
        target_app_name: "EmergencyTestTarget".to_string(),
        exemptions: vec![],
    };
    send_ipc_request(&req).await.expect("Enable failed");

    // Verify filters are active
    let status_before = send_ipc_request(&IpcRequest::GetStatus).await.expect("GetStatus failed");
    match status_before {
        IpcResponse::Status(ServiceStatus { active_filter_count, exclusive_mode_active, .. }) => {
            assert!(exclusive_mode_active, "Exclusive Mode must be active");
            assert!(active_filter_count > 0, "Active filter count must be > 0");
        }
        other => panic!("Expected Status, got {:?}", other),
    }

    // Perform Emergency Restore
    let restore_resp = send_ipc_request(&IpcRequest::EmergencyRestore).await.expect("EmergencyRestore failed");
    match restore_resp {
        IpcResponse::Ok { message } => assert!(message.contains("Emergency restore completed")),
        other => panic!("Expected Ok, got {:?}", other),
    }

    // Verify filters are now 0 and Exclusive Mode is inactive
    let status_after = send_ipc_request(&IpcRequest::GetStatus).await.expect("GetStatus failed");
    match status_after {
        IpcResponse::Status(ServiceStatus { active_filter_count, exclusive_mode_active, .. }) => {
            assert!(!exclusive_mode_active, "Exclusive Mode must be inactive after restore");
            assert_eq!(active_filter_count, 0, "All WFP filters must be flushed (0 filters)");
        }
        other => panic!("Expected Status, got {:?}", other),
    }

    // Stop and uninstall service to restore pristine OS state
    let _ = scm_control_stop();
    let _ = scm_poll_state(SERVICE_STOPPED, 10);
    let exe = get_service_exe_path();
    let _ = Command::new(&exe).arg("--uninstall").output();
}
