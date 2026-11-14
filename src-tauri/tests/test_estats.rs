use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, TCP_TABLE_OWNER_PID_ALL,
    GetPerTcpConnectionEStats, SetPerTcpConnectionEStats,
    TCP_ESTATS_TYPE, MIB_TCPROW_LH,
};
use windows_sys::Win32::Networking::WinSock::AF_INET;

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

#[test]
fn test_query_per_tcp_estats() {
    let mut size = 0u32;
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
            let res = GetExtendedTcpTable(
                buffer.as_mut_ptr() as *mut _,
                &mut size,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );

            assert_eq!(res, 0);
            let num_entries = *(buffer.as_ptr() as *const u32) as usize;
            println!("Total TCP connections found: {}", num_entries);

            let table_ptr = buffer.as_ptr().add(std::mem::size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID_RAW;
            let rows = std::slice::from_raw_parts(table_ptr, num_entries);

            let mut checked = 0;
            for row in rows {
                if row.state == 5 { // ESTABLISHED
                    let mut mib_row: MIB_TCPROW_LH = std::mem::zeroed();
                    mib_row.Anonymous.dwState = row.state;
                    mib_row.dwLocalAddr = row.local_addr;
                    mib_row.dwLocalPort = row.local_port;
                    mib_row.dwRemoteAddr = row.remote_addr;
                    mib_row.dwRemotePort = row.remote_port;

                    // Enable stats
                    let mut rw = TCP_ESTATS_DATA_RW_v0 { enable_collection: 1 };
                    let _ = SetPerTcpConnectionEStats(
                        &mib_row,
                        0, // TcpConnectionEstatsData
                        &mut rw as *mut _ as *mut u8,
                        0,
                        std::mem::size_of::<TCP_ESTATS_DATA_RW_v0>() as u32,
                        0,
                    );

                    // Read stats
                    let mut rod: TCP_ESTATS_DATA_ROD_v0 = std::mem::zeroed();
                    let err = GetPerTcpConnectionEStats(
                        &mib_row,
                        0, // TcpConnectionEstatsData
                        std::ptr::null_mut(),
                        0,
                        0,
                        std::ptr::null_mut(),
                        0,
                        0,
                        &mut rod as *mut _ as *mut u8,
                        0,
                        std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>() as u32,
                    );

                    if err == 0 {
                        checked += 1;
                        println!(
                            "PID {} (remote port {}) -> Bytes In: {}, Bytes Out: {}",
                            row.owning_pid,
                            u16::from_be(row.remote_port as u16),
                            rod.data_bytes_in,
                            rod.data_bytes_out
                        );
                    }
                }
            }
            println!("Successfully read EStats from {} established TCP connections", checked);
        }
    }
}
