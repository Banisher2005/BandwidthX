use std::collections::HashMap;
use std::time::Instant;
use parking_lot::Mutex;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetIfTable2, FreeMibTable, GetExtendedTcpTable, GetExtendedUdpTable,
    SetTcpEntry, GetPerTcpConnectionEStats, SetPerTcpConnectionEStats,
    MIB_IF_TABLE2, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
    MIB_TCPROW_LH,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use bandwidthx_common::GlobalNetworkStats;

pub struct NetworkTelemetryEngine {
    last_sample_time: Mutex<Option<Instant>>,
    last_in_bytes: Mutex<u64>,
    last_out_bytes: Mutex<u64>,
    cached_stats: Mutex<GlobalNetworkStats>,
    // Socket EStats tracking: (local_addr, local_port, remote_addr, remote_port) -> (last_in, last_out, last_time)
    socket_estats_cache: Mutex<HashMap<(u32, u32, u32, u32), (u64, u64, Instant)>>,
    // Measured traffic per PID from EStats
    measured_pid_traffic: Mutex<HashMap<u32, (u64, u64)>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SocketKey {
    pub local_addr: u32,
    pub local_port: u16,
    pub remote_addr: u32,
    pub remote_port: u16,
}

#[derive(Debug, Clone)]
pub struct SocketInfo {
    pub pid: u32,
    pub protocol: &'static str,
    pub is_ipv6: bool,
    pub local_port: u16,
    pub remote_port: u16,
    pub state: String,
    pub raw_local_addr: u32,
    pub raw_remote_addr: u32,
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

#[repr(C)]
struct MIB_UDPROW_OWNER_PID_RAW {
    local_addr: u32,
    local_port: u32,
    owning_pid: u32,
}

#[repr(C)]
struct MIB_UDP6ROW_OWNER_PID_RAW {
    local_addr: [u8; 16],
    local_scope_id: u32,
    local_port: u32,
    owning_pid: u32,
}

#[repr(C)]
struct TCP_ESTATS_DATA_ROD_v0 {
    data_bytes_out: u64,
    data_bytes_in: u64,
    segs_out: u64,
    segs_in: u64,
    soft_errors: u32,
    soft_error_reason: u32,
}

#[repr(C)]
struct TCP_ESTATS_DATA_RW_v0 {
    enable_collection: u8,
}

impl NetworkTelemetryEngine {
    pub fn new() -> Self {
        Self {
            last_sample_time: Mutex::new(None),
            last_in_bytes: Mutex::new(0),
            last_out_bytes: Mutex::new(0),
            cached_stats: Mutex::new(GlobalNetworkStats {
                download_speed_bps: 0,
                upload_speed_bps: 0,
                app_download_bps: 0,
                app_upload_bps: 0,
                total_download_bytes: 0,
                total_upload_bytes: 0,
                active_connections: 0,
                detected_applications: 0,
                adapter_name: String::from("Windows Primary Adapter"),
                adapter_type: String::from("Ethernet/Wi-Fi"),
                is_connected: true,
            }),
            socket_estats_cache: Mutex::new(HashMap::new()),
            measured_pid_traffic: Mutex::new(HashMap::new()),
        }
    }

    /// Queries real-time interface statistics from Windows GetIfTable2
    pub fn poll_global_stats(&self) -> GlobalNetworkStats {
        let mut table_ptr: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        
        let mut total_in: u64 = 0;
        let mut total_out: u64 = 0;
        let mut primary_adapter_name = String::from("Ethernet / Wi-Fi Adapter");
        let mut primary_adapter_type = String::from("Physical");
        let mut is_connected = false;

        unsafe {
            let res = GetIfTable2(&mut table_ptr);
            if res == 0 && !table_ptr.is_null() {
                let table = &*table_ptr;
                let num_entries = table.NumEntries as usize;
                let rows_slice = std::slice::from_raw_parts(table.Table.as_ptr(), num_entries);

                for row in rows_slice {
                    // Filter out loopback (IF_TYPE_SOFTWARE_LOOPBACK = 24)
                    // IF_TYPE_ETHERNET_CSMACD = 6, IF_TYPE_IEEE80211 = 71
                    if row.Type != 24 && row.OperStatus == 1 { // 1 = IfOperStatusUp
                        total_in += row.InOctets;
                        total_out += row.OutOctets;
                        is_connected = true;

                        // Capture primary adapter name from Alias
                        let alias_len = row.Alias.iter().position(|&c| c == 0).unwrap_or(row.Alias.len());
                        let alias = String::from_utf16_lossy(&row.Alias[..alias_len]);
                        if !alias.is_empty() && (primary_adapter_name.contains("Primary") || primary_adapter_name.contains("Ethernet / Wi-Fi")) {
                            primary_adapter_name = alias;
                            primary_adapter_type = match row.Type {
                                6 => String::from("Ethernet 802.3"),
                                71 => String::from("Wi-Fi 802.11"),
                                _ => String::from("Network Interface"),
                            };
                        }
                    }
                }

                FreeMibTable(table_ptr as *const _);
            }
        }

        // Calculate delta throughput
        let now = Instant::now();
        let mut last_time = self.last_sample_time.lock();
        let mut last_in = self.last_in_bytes.lock();
        let mut last_out = self.last_out_bytes.lock();

        let mut down_bps = 0;
        let mut up_bps = 0;

        if let Some(prev_time) = *last_time {
            let elapsed = now.duration_since(prev_time).as_secs_f64();
            if elapsed > 0.05 {
                if total_in >= *last_in && *last_in > 0 {
                    down_bps = ((total_in - *last_in) as f64 / elapsed) as u64;
                }
                if total_out >= *last_out && *last_out > 0 {
                    up_bps = ((total_out - *last_out) as f64 / elapsed) as u64;
                }
            }
        }

        *last_time = Some(now);
        *last_in = total_in;
        *last_out = total_out;

        // Count sockets (IPv4 + IPv6)
        let (tcp_count, _) = self.get_active_socket_counts();

        // Query actual per-process traffic via EStats
        let (app_down, app_up) = self.sample_estats_traffic();

        let stats = GlobalNetworkStats {
            download_speed_bps: down_bps,
            upload_speed_bps: up_bps,
            app_download_bps: app_down,
            app_upload_bps: app_up,
            total_download_bytes: total_in,
            total_upload_bytes: total_out,
            active_connections: tcp_count,
            detected_applications: 0,
            adapter_name: primary_adapter_name,
            adapter_type: primary_adapter_type,
            is_connected,
        };

        *self.cached_stats.lock() = stats.clone();
        stats
    }

    /// Queries TCP EStats on active connections and aggregates genuine bytes by PID
    fn sample_estats_traffic(&self) -> (u64, u64) {
        let now = Instant::now();
        let mut size = 0u32;
        let mut pid_traffic: HashMap<u32, (u64, u64)> = HashMap::new();
        let mut cache = self.socket_estats_cache.lock();

        unsafe {
            let _ = GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );

            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetExtendedTcpTable(
                    buffer.as_mut_ptr() as *mut _,
                    &mut size,
                    0,
                    AF_INET as u32,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                ) == 0 {
                    let num_entries = *(buffer.as_ptr() as *const u32) as usize;
                    let table_ptr = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID_RAW;
                    let rows = std::slice::from_raw_parts(table_ptr, num_entries);

                    for row in rows {
                        if row.state == 5 { // ESTABLISHED
                            let mut mib_row: MIB_TCPROW_LH = std::mem::zeroed();
                            mib_row.Anonymous.dwState = row.state;
                            mib_row.dwLocalAddr = row.local_addr;
                            mib_row.dwLocalPort = row.local_port;
                            mib_row.dwRemoteAddr = row.remote_addr;
                            mib_row.dwRemotePort = row.remote_port;

                            // Ensure EStats is enabled on this socket
                            let mut rw = TCP_ESTATS_DATA_RW_v0 { enable_collection: 1 };
                            let _ = SetPerTcpConnectionEStats(
                                &mib_row,
                                0, // TcpConnectionEstatsData
                                &mut rw as *mut _ as *mut u8,
                                0,
                                std::mem::size_of::<TCP_ESTATS_DATA_RW_v0>() as u32,
                                0,
                            );

                            let mut rod: TCP_ESTATS_DATA_ROD_v0 = std::mem::zeroed();
                            if GetPerTcpConnectionEStats(
                                &mib_row,
                                0,
                                std::ptr::null_mut(),
                                0,
                                0,
                                std::ptr::null_mut(),
                                0,
                                0,
                                &mut rod as *mut _ as *mut u8,
                                0,
                                std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>() as u32,
                            ) == 0 {
                                let key = (row.local_addr, row.local_port, row.remote_addr, row.remote_port);
                                if let Some((prev_in, prev_out, prev_time)) = cache.get(&key) {
                                    let dt = now.duration_since(*prev_time).as_secs_f64();
                                    if dt > 0.05 {
                                        let delta_in = if rod.data_bytes_in >= *prev_in {
                                            ((rod.data_bytes_in - *prev_in) as f64 / dt) as u64
                                        } else { 0 };

                                        let delta_out = if rod.data_bytes_out >= *prev_out {
                                            ((rod.data_bytes_out - *prev_out) as f64 / dt) as u64
                                        } else { 0 };

                                        let entry = pid_traffic.entry(row.owning_pid).or_insert((0, 0));
                                        entry.0 += delta_in;
                                        entry.1 += delta_out;
                                    }
                                }
                                cache.insert(key, (rod.data_bytes_in, rod.data_bytes_out, now));
                            }
                        }
                    }
                }
            }
        }

        // Clean up stale sockets from cache (older than 10 seconds)
        cache.retain(|_, (_, _, t)| now.duration_since(*t).as_secs() < 10);

        let mut total_app_down = 0u64;
        let mut total_app_up = 0u64;
        for (d, u) in pid_traffic.values() {
            total_app_down += *d;
            total_app_up += *u;
        }

        *self.measured_pid_traffic.lock() = pid_traffic;
        (total_app_down, total_app_up)
    }

    /// Returns actual measured per-PID traffic (download_bps, upload_bps)
    pub fn get_measured_pid_traffic(&self) -> HashMap<u32, (u64, u64)> {
        self.measured_pid_traffic.lock().clone()
    }

    /// Forcefully terminates an established TCP connection via SetTcpEntry (MIB_TCP_STATE_DELETE_TCB)
    pub fn terminate_tcp_connection(local_addr: u32, local_port: u16, remote_addr: u32, remote_port: u16) -> bool {
        unsafe {
            let mut row: MIB_TCPROW_LH = std::mem::zeroed();
            row.Anonymous.dwState = 12; // MIB_TCP_STATE_DELETE_TCB
            row.dwLocalAddr = local_addr;
            row.dwLocalPort = (local_port as u32).to_be();
            row.dwRemoteAddr = remote_addr;
            row.dwRemotePort = (remote_port as u32).to_be();

            SetTcpEntry(&row) == 0
        }
    }

    /// Retrieves all active TCP and UDP sockets (both IPv4 and IPv6) grouped by owning PID
    pub fn get_sockets_by_pid(&self) -> HashMap<u32, Vec<SocketInfo>> {
        let mut map: HashMap<u32, Vec<SocketInfo>> = HashMap::new();

        // 1. IPv4 TCP Table
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_ALL, 0);
            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetExtendedTcpTable(buffer.as_mut_ptr() as *mut _, &mut size, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_ALL, 0) == 0 {
                    let num = *(buffer.as_ptr() as *const u32) as usize;
                    let table = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID_RAW;
                    for row in std::slice::from_raw_parts(table, num) {
                        let state = parse_tcp_state(row.state);
                        map.entry(row.owning_pid).or_default().push(SocketInfo {
                            pid: row.owning_pid,
                            protocol: "TCP",
                            is_ipv6: false,
                            local_port: u16::from_be(row.local_port as u16),
                            remote_port: u16::from_be(row.remote_port as u16),
                            state: state.to_string(),
                            raw_local_addr: row.local_addr,
                            raw_remote_addr: row.remote_addr,
                        });
                    }
                }
            }
        }

        // 2. IPv6 TCP Table
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET6 as u32, TCP_TABLE_OWNER_PID_ALL, 0);
            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetExtendedTcpTable(buffer.as_mut_ptr() as *mut _, &mut size, 0, AF_INET6 as u32, TCP_TABLE_OWNER_PID_ALL, 0) == 0 {
                    let num = *(buffer.as_ptr() as *const u32) as usize;
                    let table = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCP6ROW_OWNER_PID_RAW;
                    for row in std::slice::from_raw_parts(table, num) {
                        let state = parse_tcp_state(row.state);
                        map.entry(row.owning_pid).or_default().push(SocketInfo {
                            pid: row.owning_pid,
                            protocol: "TCP",
                            is_ipv6: true,
                            local_port: u16::from_be(row.local_port as u16),
                            remote_port: u16::from_be(row.remote_port as u16),
                            state: state.to_string(),
                            raw_local_addr: 0,
                            raw_remote_addr: 0,
                        });
                    }
                }
            }
        }

        // 3. IPv4 UDP Table
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedUdpTable(std::ptr::null_mut(), &mut size, 0, AF_INET as u32, UDP_TABLE_OWNER_PID, 0);
            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetExtendedUdpTable(buffer.as_mut_ptr() as *mut _, &mut size, 0, AF_INET as u32, UDP_TABLE_OWNER_PID, 0) == 0 {
                    let num = *(buffer.as_ptr() as *const u32) as usize;
                    let table = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_UDPROW_OWNER_PID_RAW;
                    for row in std::slice::from_raw_parts(table, num) {
                        map.entry(row.owning_pid).or_default().push(SocketInfo {
                            pid: row.owning_pid,
                            protocol: "UDP",
                            is_ipv6: false,
                            local_port: u16::from_be(row.local_port as u16),
                            remote_port: 0,
                            state: "LISTENING".to_string(),
                            raw_local_addr: row.local_addr,
                            raw_remote_addr: 0,
                        });
                    }
                }
            }
        }

        // 4. IPv6 UDP Table
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedUdpTable(std::ptr::null_mut(), &mut size, 0, AF_INET6 as u32, UDP_TABLE_OWNER_PID, 0);
            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetExtendedUdpTable(buffer.as_mut_ptr() as *mut _, &mut size, 0, AF_INET6 as u32, UDP_TABLE_OWNER_PID, 0) == 0 {
                    let num = *(buffer.as_ptr() as *const u32) as usize;
                    let table = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_UDP6ROW_OWNER_PID_RAW;
                    for row in std::slice::from_raw_parts(table, num) {
                        map.entry(row.owning_pid).or_default().push(SocketInfo {
                            pid: row.owning_pid,
                            protocol: "UDP",
                            is_ipv6: true,
                            local_port: u16::from_be(row.local_port as u16),
                            remote_port: 0,
                            state: "LISTENING".to_string(),
                            raw_local_addr: 0,
                            raw_remote_addr: 0,
                        });
                    }
                }
            }
        }

        map
    }

    pub fn get_active_socket_counts(&self) -> (usize, usize) {
        let sockets = self.get_sockets_by_pid();
        let mut tcp = 0;
        let mut udp = 0;
        for list in sockets.values() {
            for s in list {
                if s.protocol == "TCP" { tcp += 1; } else { udp += 1; }
            }
        }
        (tcp, udp)
    }
}

fn parse_tcp_state(state: u32) -> &'static str {
    match state {
        1 => "CLOSED",
        2 => "LISTEN",
        3 => "SYN_SENT",
        4 => "SYN_RCVD",
        5 => "ESTABLISHED",
        6 => "FIN_WAIT1",
        7 => "FIN_WAIT2",
        8 => "CLOSE_WAIT",
        9 => "CLOSING",
        10 => "LAST_ACK",
        11 => "TIME_WAIT",
        12 => "DELETE_TCB",
        _ => "UNKNOWN",
    }
}
