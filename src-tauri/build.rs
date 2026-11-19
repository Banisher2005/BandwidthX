fn main() {
    let res_dir = std::path::Path::new("resources");
    if !res_dir.exists() {
        let _ = std::fs::create_dir_all(res_dir);
    }
    let service_path = res_dir.join("bandwidthx-service.exe");
    if !service_path.exists() {
        let _ = std::fs::write(&service_path, b"");
    }

    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("windows/app.manifest"));

    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
