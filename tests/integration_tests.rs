use std::collections::HashMap;

#[test]
fn test_guid_parsing() {
    let guid_str = "87b4699e-5bdf-4cb8-9b88-14f77c862bc1";
    let clean = guid_str.replace('-', "");
    assert_eq!(clean.len(), 32);
}

#[test]
fn test_bandwidth_formatting() {
    fn format_speed(bytes_per_sec: u64) -> String {
        if bytes_per_sec == 0 {
            return "0 B/s".to_string();
        }
        let k = 1024.0;
        let sizes = ["B/s", "KB/s", "MB/s", "GB/s"];
        let i = (bytes_per_sec as f64).log(k).floor() as usize;
        let val = (bytes_per_sec as f64) / k.powi(i as i32);
        format!("{:.1} {}", val, sizes[i])
    }

    assert_eq!(format_speed(0), "0 B/s");
    assert_eq!(format_speed(1024), "1.0 KB/s");
    assert_eq!(format_speed(1048576), "1.0 MB/s");
    assert_eq!(format_speed(52428800), "50.0 MB/s");
}

#[test]
fn test_socket_aggregation_logic() {
    let mut sockets: HashMap<u32, usize> = HashMap::new();
    sockets.insert(1001, 10);
    sockets.insert(1002, 30);

    let total: usize = sockets.values().sum();
    assert_eq!(total, 40);

    let proc1_ratio = *sockets.get(&1001).unwrap() as f64 / total as f64;
    assert_eq!(proc1_ratio, 0.25);

    let global_down_bps = 1_000_000u64;
    let proc1_down = (global_down_bps as f64 * proc1_ratio) as u64;
    assert_eq!(proc1_down, 250_000);
}
