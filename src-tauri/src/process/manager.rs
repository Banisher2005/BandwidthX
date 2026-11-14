use std::collections::HashMap;
use parking_lot::Mutex;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, FILETIME};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, GetProcessTimes
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use bandwidthx_common::ProcessNetworkInfo;
use crate::network::iphelper::SocketInfo;

struct TrackedProcess {
    creation_time: u64,
    exe_path: String,
    total_download: u64,
    total_upload: u64,
}

pub struct ProcessManager {
    // Map of PID -> TrackedProcess (for PID reuse protection and cumulative totals)
    process_tracker: Mutex<HashMap<u32, TrackedProcess>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            process_tracker: Mutex::new(HashMap::new()),
        }
    }

    /// Queries all running processes, validates process lifetime against PID reuse,
    /// and associates them with measured socket telemetry.
    pub fn get_processes_with_network(
        &self,
        sockets_by_pid: &HashMap<u32, Vec<SocketInfo>>,
        measured_traffic: &HashMap<u32, (u64, u64)>,
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

        let mut current_snapshot_pids = Vec::new();
        let mut proc_names: HashMap<u32, String> = HashMap::new();

        unsafe {
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let pid = entry.th32ProcessID;
                    let name_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);

                    current_snapshot_pids.push(pid);
                    proc_names.insert(pid, name);

                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
        }

        let mut tracker = self.process_tracker.lock();

        // Evict processes from tracker that no longer exist (process termination handling)
        let current_pid_set: std::collections::HashSet<u32> = current_snapshot_pids.iter().copied().collect();
        tracker.retain(|pid, _| current_pid_set.contains(pid));

        for pid in current_snapshot_pids {
            let sockets = sockets_by_pid.get(&pid);
            let socket_count = sockets.map(|s| s.len()).unwrap_or(0);

            // Query process creation time to protect against PID reuse
            let (creation_time, exe_path) = self.get_process_identity(pid);

            let is_exclusive = exclusive_target_path.map_or(false, |t| !t.is_empty() && exe_path.eq_ignore_ascii_case(t));
            let is_blocked = blocked_paths.iter().any(|b| exe_path.eq_ignore_ascii_case(b));

            // Skip processes with zero sockets unless they are blocked or targeted
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

            // Actual measured traffic from EStats / socket telemetry (NEVER fabricated)
            let (down_sec, up_sec) = measured_traffic.get(&pid).copied().unwrap_or((0, 0));

            // Check PID reuse
            let tracked = tracker.entry(pid).or_insert_with(|| TrackedProcess {
                creation_time,
                exe_path: exe_path.clone(),
                total_download: 0,
                total_upload: 0,
            });

            // If creation time changed, PID was recycled by the OS!
            if tracked.creation_time != 0 && creation_time != 0 && tracked.creation_time != creation_time {
                tracked.creation_time = creation_time;
                tracked.exe_path = exe_path.clone();
                tracked.total_download = 0;
                tracked.total_upload = 0;
            }

            tracked.total_download += down_sec;
            tracked.total_upload += up_sec;

            processes.push(ProcessNetworkInfo {
                pid,
                name,
                exe_path,
                download_bytes_sec: down_sec,
                upload_bytes_sec: up_sec,
                total_download_bytes: tracked.total_download,
                total_upload_bytes: tracked.total_upload,
                connection_count: socket_count,
                is_exclusive_target: is_exclusive,
                is_blocked,
                has_rules: is_blocked,
                active_tcp_connections: tcp_count,
                active_udp_endpoints: udp_count,
            });
        }

        // Sort: Active traffic first, then total connection count descending
        processes.sort_by(|a, b| {
            let speed_a = a.download_bytes_sec + a.upload_bytes_sec;
            let speed_b = b.download_bytes_sec + b.upload_bytes_sec;
            if speed_b != speed_a {
                speed_b.cmp(&speed_a)
            } else {
                b.connection_count.cmp(&a.connection_count)
            }
        });

        processes
    }

    /// Resolves full executable path and creation time for a process with PID reuse protection
    pub fn get_process_identity(&self, pid: u32) -> (u64, String) {
        if pid == 0 {
            return (0, String::from("System Idle Process"));
        }
        if pid == 4 {
            return (0, String::from("System"));
        }

        unsafe {
            let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // Inaccessible or protected process (e.g. audiodg.exe, anti-cheat, protected kernel service)
                return (0, format!("C:\\Windows\\System32\\process_{}.exe", pid));
            }

            // 1. Get process creation time
            let mut creation_time: FILETIME = std::mem::zeroed();
            let mut exit_time: FILETIME = std::mem::zeroed();
            let mut kernel_time: FILETIME = std::mem::zeroed();
            let mut user_time: FILETIME = std::mem::zeroed();

            let create_ts = if GetProcessTimes(handle, &mut creation_time, &mut exit_time, &mut kernel_time, &mut user_time) != 0 {
                ((creation_time.dwHighDateTime as u64) << 32) | (creation_time.dwLowDateTime as u64)
            } else {
                0
            };

            // 2. Query full image name
            let mut path_buf = [0u16; 1024];
            let mut size: u32 = path_buf.len() as u32;
            let success = QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size);
            CloseHandle(handle);

            let path = if success != 0 && size > 0 {
                String::from_utf16_lossy(&path_buf[..size as usize])
            } else {
                format!("Process_{}.exe", pid)
            };

            (create_ts, path)
        }
    }

    pub fn get_process_path(&self, pid: u32) -> String {
        self.get_process_identity(pid).1
    }
}
