use std::collections::HashMap;
use std::time::Instant;
use parking_lot::Mutex;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetIfTable2, FreeMibTable, GetExtendedTcpTable, GetExtendedUdpTable,
    MIB_IF_TABLE2, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::Networking::WinSock::AF_INET;
use bandwidthx_common::GlobalNetworkStats;

pub struct NetworkTelemetryEngine {
    last_sample_time: Mutex<Option<Instant>>,
    last_in_bytes: Mutex<u64>,
    last_out_bytes: Mutex<u64>,
    cached_stats: Mutex<GlobalNetworkStats>,
}

#[derive(Debug, Clone)]
pub struct SocketInfo {
    pub pid: u32,
    pub protocol: &'static str,
    pub local_port: u16,
    pub remote_port: u16,
    pub state: String,
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
                total_download_bytes: 0,
                total_upload_bytes: 0,
                active_connections: 0,
                detected_applications: 0,
                adapter_name: String::from("Windows Primary Adapter"),
                adapter_type: String::from("Ethernet/Wi-Fi"),
                is_connected: true,
            }),
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

        // Count sockets
        let (tcp_count, _) = self.get_active_socket_counts();

        let stats = GlobalNetworkStats {
            download_speed_bps: down_bps,
            upload_speed_bps: up_bps,
            total_download_bytes: total_in,
            total_upload_bytes: total_out,
            active_connections: tcp_count,
            detected_applications: 0, // Populated by process manager
            adapter_name: primary_adapter_name,
            adapter_type: primary_adapter_type,
            is_connected,
        };

        *self.cached_stats.lock() = stats.clone();
        stats
    }

    /// Retrieves all active TCP and UDP sockets grouped by owning PID
    pub fn get_sockets_by_pid(&self) -> HashMap<u32, Vec<SocketInfo>> {
        let mut map: HashMap<u32, Vec<SocketInfo>> = HashMap::new();

        // 1. Get IPv4 TCP Table
        unsafe {
            let mut size: u32 = 0;
            // First call to determine buffer size
            let _ = GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0, // Sort = false
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );

            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                let res = GetExtendedTcpTable(
                    buffer.as_mut_ptr() as *mut _,
                    &mut size,
                    0,
                    AF_INET as u32,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                );

                if res == 0 {
                    let num_entries = *(buffer.as_ptr() as *const u32) as usize;
                    let table_ptr = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID_RAW;
                    let rows = std::slice::from_raw_parts(table_ptr, num_entries);

                    for row in rows {
                        let state_str = match row.state {
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
                        };

                        let socket = SocketInfo {
                            pid: row.owning_pid,
                            protocol: "TCP",
                            local_port: u16::from_be(row.local_port as u16),
                            remote_port: u16::from_be(row.remote_port as u16),
                            state: state_str.to_string(),
                        };

                        map.entry(row.owning_pid).or_default().push(socket);
                    }
                }
            }
        }

        // 2. Get IPv4 UDP Table
        unsafe {
            let mut size: u32 = 0;
            let _ = GetExtendedUdpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                AF_INET as u32,
                UDP_TABLE_OWNER_PID,
                0,
            );

            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                let res = GetExtendedUdpTable(
                    buffer.as_mut_ptr() as *mut _,
                    &mut size,
                    0,
                    AF_INET as u32,
                    UDP_TABLE_OWNER_PID,
                    0,
                );

                if res == 0 {
                    let num_entries = *(buffer.as_ptr() as *const u32) as usize;
                    let table_ptr = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_UDPROW_OWNER_PID_RAW;
                    let rows = std::slice::from_raw_parts(table_ptr, num_entries);

                    for row in rows {
                        let socket = SocketInfo {
                            pid: row.owning_pid,
                            protocol: "UDP",
                            local_port: u16::from_be(row.local_port as u16),
                            remote_port: 0,
                            state: "LISTENING".to_string(),
                        };

                        map.entry(row.owning_pid).or_default().push(socket);
                    }
                }
            }
        }

        map
    }

    pub fn get_active_socket_counts(&self) -> (usize, usize) {
        let sockets = self.get_sockets_by_pid();
        let mut tcp_count = 0;
        let mut udp_count = 0;

        for s_list in sockets.values() {
            for s in s_list {
                if s.protocol == "TCP" {
                    tcp_count += 1;
                } else {
                    udp_count += 1;
                }
            }
        }
        (tcp_count, udp_count)
    }
}

// C-compatible raw struct representations for Extended TCP/UDP tables
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
struct MIB_UDPROW_OWNER_PID_RAW {
    local_addr: u32,
    local_port: u32,
    owning_pid: u32,
}
