use std::collections::HashMap;
use parking_lot::Mutex;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use bandwidthx_common::ProcessNetworkInfo;
use crate::network::iphelper::SocketInfo;

pub struct ProcessManager {
    cumulative_bytes: Mutex<HashMap<u32, (u64, u64)>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            cumulative_bytes: Mutex::new(HashMap::new()),
        }
    }

    /// Queries all running processes and merges them with active network sockets
    pub fn get_processes_with_network(
        &self,
        sockets_by_pid: &HashMap<u32, Vec<SocketInfo>>,
        global_down_bps: u64,
        global_up_bps: u64,
        exclusive_target_path: Option<&str>,
        blocked_paths: &[String],
    ) -> Vec<ProcessNetworkInfo> {
        let mut processes = Vec::new();

        // 1. Snapshot running processes
        let snapshot: HANDLE = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if (snapshot as isize) == -1 {
            return processes;
        }

        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut all_pids = Vec::new();
        let mut proc_names: HashMap<u32, String> = HashMap::new();

        unsafe {
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let pid = entry.th32ProcessID;
                    let name_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);

                    all_pids.push(pid);
                    proc_names.insert(pid, name);

                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
        }

        // Count total active sockets for proportional bandwidth distribution
        let total_active_sockets: usize = sockets_by_pid.values().map(|v| v.len()).sum();
        let mut cum_map = self.cumulative_bytes.lock();

        // Process only PIDs that have active network sockets or are common desktop apps
        for pid in all_pids {
            let sockets = sockets_by_pid.get(&pid);
            let socket_count = sockets.map(|s| s.len()).unwrap_or(0);

            // Skip processes with 0 sockets unless they are blocked or target
            let exe_path = self.get_process_path(pid);
            let is_exclusive = exclusive_target_path.map_or(false, |t| !t.is_empty() && exe_path.eq_ignore_ascii_case(t));
            let is_blocked = blocked_paths.iter().any(|b| exe_path.eq_ignore_ascii_case(b));

            if socket_count == 0 && !is_exclusive && !is_blocked {
                continue;
            }

            let name = proc_names.get(&pid).cloned().unwrap_or_else(|| format!("Process_{}", pid));

            let (tcp_count, udp_count) = sockets.map_or((0, 0), |s_list| {
                let mut tcp = 0;
                let mut udp = 0;
                for s in s_list {
                    if s.protocol == "TCP" { tcp += 1; } else { udp += 1; }
                }
                (tcp, udp)
            });

            // Calculate estimated speed based on socket proportion
            let (down_sec, up_sec) = if total_active_sockets > 0 && socket_count > 0 {
                let ratio = socket_count as f64 / total_active_sockets as f64;
                (
                    (global_down_bps as f64 * ratio) as u64,
                    (global_up_bps as f64 * ratio) as u64,
                )
            } else {
                (0, 0)
            };

            // Accumulate bytes
            let cum_entry = cum_map.entry(pid).or_insert((0, 0));
            cum_entry.0 += down_sec;
            cum_entry.1 += up_sec;

            processes.push(ProcessNetworkInfo {
                pid,
                name,
                exe_path,
                download_bytes_sec: down_sec,
                upload_bytes_sec: up_sec,
                total_download_bytes: cum_entry.0,
                total_upload_bytes: cum_entry.1,
                connection_count: socket_count,
                is_exclusive_target: is_exclusive,
                is_blocked,
                has_rules: is_blocked,
                active_tcp_connections: tcp_count,
                active_udp_endpoints: udp_count,
            });
        }

        // Sort by total bandwidth descending
        processes.sort_by(|a, b| {
            let speed_a = a.download_bytes_sec + a.upload_bytes_sec;
            let speed_b = b.download_bytes_sec + b.upload_bytes_sec;
            speed_b.cmp(&speed_a)
        });

        processes
    }

    /// Resolves full executable path for a process ID using QueryFullProcessImageNameW
    pub fn get_process_path(&self, pid: u32) -> String {
        if pid == 0 {
            return String::from("System Idle Process");
        }
        if pid == 4 {
            return String::from("System");
        }

        unsafe {
            let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return format!("C:\\Windows\\System32\\process_{}.exe", pid);
            }

            let mut path_buf = [0u16; 1024];
            let mut size: u32 = path_buf.len() as u32;

            let success = QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size);
            CloseHandle(handle);

            if success != 0 && size > 0 {
                String::from_utf16_lossy(&path_buf[..size as usize])
            } else {
                format!("Process_{}.exe", pid)
            }
        }
    }
}
