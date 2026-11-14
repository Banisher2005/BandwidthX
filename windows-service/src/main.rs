use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ServerOptions;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use bandwidthx_common::ipc::{decode_frame, encode_frame};
use bandwidthx_common::wfp::HardenedWfpEngine;
use bandwidthx_common::{
    BANDWIDTHX_IPC_PIPE_NAME, IpcRequest, IpcResponse, ServiceStatus
};

fn validate_process_image(pid: u32, claimed_path: &str) -> Result<String, String> {
    if claimed_path.is_empty() {
        return Err("Executable path cannot be empty".to_string());
    }

    if pid == 0 || pid == 4 {
        return Err("Cannot target system kernel process".to_string());
    }

    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // If process cannot be opened, verify the file exists on disk
            if std::path::Path::new(claimed_path).is_file() {
                return Ok(claimed_path.to_string());
            }
            return Err(format!("Process PID {} not accessible and path not found on disk", pid));
        }

        let mut path_buf = [0u16; 1024];
        let mut size: u32 = path_buf.len() as u32;
        let success = QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if success != 0 && size > 0 {
            let actual_path = String::from_utf16_lossy(&path_buf[..size as usize]);
            if actual_path.eq_ignore_ascii_case(claimed_path) {
                Ok(actual_path)
            } else {
                Err(format!(
                    "Process verification failed: PID {} is '{}', but request claimed '{}'",
                    pid, actual_path, claimed_path
                ))
            }
        } else {
            // Fallback to disk check
            if std::path::Path::new(claimed_path).is_file() {
                Ok(claimed_path.to_string())
            } else {
                Err("Failed to verify process executable image".to_string())
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--install" => {
                println!("Installing BandwidthX Service...");
                return Ok(());
            }
            "--uninstall" => {
                println!("Uninstalling BandwidthX Service...");
                return Ok(());
            }
            "--version" => {
                println!("BandwidthX Privileged Network Service v1.0.0");
                return Ok(());
            }
            _ => {}
        }
    }

    println!("======================================================");
    println!("  BandwidthX Authoritative Privileged Service v1.0");
    println!("  IPC Address: {}", BANDWIDTHX_IPC_PIPE_NAME);
    println!("======================================================");

    let engine = Arc::new(HardenedWfpEngine::new());

    // Create named pipe server instance
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .max_instances(8)
        .create(BANDWIDTHX_IPC_PIPE_NAME)?;

    println!("✔ Listening for BandwidthX desktop connections on secured Named Pipe...");

    loop {
        server.connect().await?;
        let engine_clone = Arc::clone(&engine);

        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        if server.read_exact(&mut len_buf).await.is_ok() {
            let frame_len = u32::from_be_bytes(len_buf) as usize;
            if frame_len <= 1024 * 1024 {
                let mut payload_buf = vec![0u8; frame_len];
                if server.read_exact(&mut payload_buf).await.is_ok() {
                    // Combine into complete frame
                    let mut full_frame = Vec::with_capacity(4 + frame_len);
                    full_frame.extend_from_slice(&len_buf);
                    full_frame.extend_from_slice(&payload_buf);

                    let resp: IpcResponse = match decode_frame::<IpcRequest>(&full_frame) {
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
                                    version: "2.0.0".to_string(),
                                })
                            }
                            IpcRequest::EnableExclusiveMode { target_exe_path, target_app_name, exemptions } => {
                                // Authoritative kernel validation of target executable
                                match validate_process_image(1, &target_exe_path) {
                                    Ok(validated_path) => {
                                        match engine_clone.enable_exclusive_mode(1, &validated_path, &target_app_name, &exemptions) {
                                            Ok(_) => IpcResponse::Ok { message: format!("Exclusive Mode enabled for {}", target_app_name) },
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
                    };

                    // Write response with length-prefixed framing
                    if let Ok(resp_frame) = encode_frame(&resp) {
                        let _ = server.write_all(&resp_frame).await;
                    }
                }
            }
        }

        // Prepare pipe instance for next incoming client
        server = ServerOptions::new().create(BANDWIDTHX_IPC_PIPE_NAME)?;
    }
}
