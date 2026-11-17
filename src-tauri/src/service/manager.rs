use std::ffi::c_void;
use serde::{Deserialize, Serialize};
use windows_sys::Win32::System::Services::{
    OpenSCManagerW, OpenServiceW, CloseServiceHandle, EnumServicesStatusExW,
    QueryServiceConfigW, ControlService, StartServiceW,
    SC_MANAGER_ENUMERATE_SERVICE, SC_MANAGER_CONNECT,
    SERVICE_START, SERVICE_STOP, SERVICE_QUERY_CONFIG,
    SERVICE_WIN32, SERVICE_STATE_ALL, SC_ENUM_PROCESS_INFO,
    ENUM_SERVICE_STATUS_PROCESSW, QUERY_SERVICE_CONFIGW,
    SERVICE_STATUS, SERVICE_CONTROL_STOP,
    SERVICE_RUNNING, SERVICE_STOPPED, SERVICE_PAUSED,
    SERVICE_START_PENDING, SERVICE_STOP_PENDING,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceSafetyClassification {
    Protected, // Core OS / Security / Network infrastructure (Cannot stop/terminate)
    System,    // Windows system service (Warn before stopping)
    User,      // Non-Microsoft / third-party application service
    Unknown,   // Unclassified
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsServiceInfo {
    pub service_name: String,
    pub display_name: String,
    pub pid: Option<u32>,
    pub exe_path: Option<String>,
    pub status: String,
    pub startup_type: String,
    pub description: String,
    pub classification: ServiceSafetyClassification,
    pub can_stop: bool,
    pub can_start: bool,
    pub can_restart: bool,
}

pub struct WindowsServiceManager;

impl WindowsServiceManager {
    pub fn new() -> Self {
        Self
    }

    /// Determines the safety classification for a Windows service.
    /// Never allows termination or stopping of critical security or networking infrastructure.
    pub fn classify_service(name: &str, exe_path: Option<&str>) -> ServiceSafetyClassification {
        let lower_name = name.to_ascii_lowercase();

        // 1. Critical Protected Windows Infrastructure
        // These services are fundamental to OS stability, security, or WFP filtering.
        let protected_services = [
            "rpcss", "dcomlaunch", "lsaps", "rpceptmapper", "samss", "eventlog",
            "plugplay", "lanmanworkstation", "lanmanserver", "dhcp", "dnscache",
            "wfpl2svc", "bfe", "mpssvc", "windefend", "securityhealthservice",
            "sppsvc", "cryptsvc", "trustedinstaller", "netprofm", "nlasvc",
            "bandwidthx", // BandwidthX itself
        ];

        if protected_services.contains(&lower_name.as_str()) {
            return ServiceSafetyClassification::Protected;
        }

        // 2. Classify based on binary path
        if let Some(path) = exe_path {
            let lower_path = path.to_ascii_lowercase();
            if lower_path.contains(r"\windows\system32\") || lower_path.contains(r"\windows\syswow64\") {
                return ServiceSafetyClassification::System;
            }
            if lower_path.contains(r"\program files") || lower_path.contains(r"\appdata\") {
                return ServiceSafetyClassification::User;
            }
        }

        ServiceSafetyClassification::Unknown
    }

    /// Enumerates all Windows services using native Win32 EnumServicesStatusExW
    pub fn list_services(&self) -> Result<Vec<WindowsServiceInfo>, String> {
        let mut services = Vec::new();

        unsafe {
            let scm = OpenSCManagerW(
                std::ptr::null(),
                std::ptr::null(),
                SC_MANAGER_ENUMERATE_SERVICE | SC_MANAGER_CONNECT,
            );

            if scm.is_null() {
                return Err(format!("OpenSCManager failed: {}", std::io::Error::last_os_error()));
            }

            let mut bytes_needed: u32 = 0;
            let mut services_returned: u32 = 0;
            let mut resume_handle: u32 = 0;

            // First call to determine buffer size
            let _ = EnumServicesStatusExW(
                scm,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                std::ptr::null_mut(),
                0,
                &mut bytes_needed,
                &mut services_returned,
                &mut resume_handle,
                std::ptr::null(),
            );

            if bytes_needed == 0 {
                CloseServiceHandle(scm);
                return Ok(services);
            }

            let mut buffer: Vec<u8> = vec![0u8; bytes_needed as usize];

            let res = EnumServicesStatusExW(
                scm,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                buffer.as_mut_ptr(),
                bytes_needed,
                &mut bytes_needed,
                &mut services_returned,
                &mut resume_handle,
                std::ptr::null(),
            );

            if res == 0 {
                let err = std::io::Error::last_os_error();
                CloseServiceHandle(scm);
                return Err(format!("EnumServicesStatusExW failed: {}", err));
            }

            let ptr = buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
            let entries = std::slice::from_raw_parts(ptr, services_returned as usize);

            for entry in entries {
                let name = wide_ptr_to_string(entry.lpServiceName);
                let display = wide_ptr_to_string(entry.lpDisplayName);

                let status_raw = entry.ServiceStatusProcess.dwCurrentState;
                let (status_str, can_start, can_stop, can_restart) = match status_raw {
                    SERVICE_RUNNING => ("Running".to_string(), false, true, true),
                    SERVICE_STOPPED => ("Stopped".to_string(), true, false, false),
                    SERVICE_PAUSED => ("Paused".to_string(), true, true, false),
                    SERVICE_START_PENDING => ("Starting...".to_string(), false, false, false),
                    SERVICE_STOP_PENDING => ("Stopping...".to_string(), false, false, false),
                    _ => ("Unknown".to_string(), false, false, false),
                };

                let pid = if entry.ServiceStatusProcess.dwProcessId != 0 {
                    Some(entry.ServiceStatusProcess.dwProcessId)
                } else {
                    None
                };

                // Query service binary path and startup type
                let (exe_path, startup_type) = query_service_details(scm, &name);

                let classification = Self::classify_service(&name, exe_path.as_deref());

                // Protected services CANNOT be stopped or restarted from the UI
                let is_protected = classification == ServiceSafetyClassification::Protected;

                services.push(WindowsServiceInfo {
                    service_name: name,
                    display_name: display,
                    pid,
                    exe_path,
                    status: status_str,
                    startup_type,
                    description: String::new(),
                    can_stop: can_stop && !is_protected,
                    can_start: can_start && !is_protected,
                    can_restart: can_restart && !is_protected,
                    classification,
                });
            }

            CloseServiceHandle(scm);
        }

        // Sort: Running services first, then alphabetical by display name
        services.sort_by(|a, b| {
            if a.status == "Running" && b.status != "Running" {
                std::cmp::Ordering::Less
            } else if a.status != "Running" && b.status == "Running" {
                std::cmp::Ordering::Greater
            } else {
                a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase())
            }
        });

        Ok(services)
    }

    /// Starts an authorized Windows service
    pub fn start_service(&self, service_name: &str) -> Result<bool, String> {
        let classification = Self::classify_service(service_name, None);
        if classification == ServiceSafetyClassification::Protected && service_name.to_lowercase() != "bandwidthx" {
            return Err("Cannot modify protected core Windows service".to_string());
        }

        let service_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
            if scm.is_null() {
                return Err("Failed to open Service Control Manager".to_string());
            }

            let service = OpenServiceW(scm, service_wide.as_ptr(), SERVICE_START);
            if service.is_null() {
                CloseServiceHandle(scm);
                return Err(format!("Access denied or service '{}' not found", service_name));
            }

            let res = StartServiceW(service, 0, std::ptr::null_mut());
            let err = std::io::Error::last_os_error();
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if res != 0 || err.raw_os_error() == Some(1056) {
                Ok(true)
            } else {
                Err(format!("StartService failed: {}", err))
            }
        }
    }

    /// Stops an authorized Windows service. Strictly rejects protected services!
    pub fn stop_service(&self, service_name: &str) -> Result<bool, String> {
        let classification = Self::classify_service(service_name, None);
        if classification == ServiceSafetyClassification::Protected {
            return Err(format!("Action blocked: '{}' is a protected Windows core service", service_name));
        }

        let service_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
            if scm.is_null() {
                return Err("Failed to open Service Control Manager".to_string());
            }

            let service = OpenServiceW(scm, service_wide.as_ptr(), SERVICE_STOP);
            if service.is_null() {
                CloseServiceHandle(scm);
                return Err(format!("Access denied or service '{}' not found", service_name));
            }

            let mut status: SERVICE_STATUS = std::mem::zeroed();
            let res = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
            let err = std::io::Error::last_os_error();
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if res != 0 || err.raw_os_error() == Some(1062) {
                Ok(true)
            } else {
                Err(format!("StopService failed: {}", err))
            }
        }
    }

    /// Restarts an authorized Windows service
    pub fn restart_service(&self, service_name: &str) -> Result<bool, String> {
        let _ = self.stop_service(service_name)?;
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.start_service(service_name)
    }
}

unsafe fn wide_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf16_lossy(slice)
}

unsafe fn query_service_details(scm: *mut c_void, service_name: &str) -> (Option<String>, String) {
    let name_wide: Vec<u16> = service_name.encode_utf16().chain(std::iter::once(0)).collect();
    let service = OpenServiceW(scm as _, name_wide.as_ptr(), SERVICE_QUERY_CONFIG);
    if service.is_null() {
        return (None, "Unknown".to_string());
    }

    let mut bytes_needed = 0u32;
    let _ = QueryServiceConfigW(service, std::ptr::null_mut(), 0, &mut bytes_needed);
    if bytes_needed == 0 {
        CloseServiceHandle(service);
        return (None, "Unknown".to_string());
    }

    let mut buf = vec![0u8; bytes_needed as usize];
    let res = QueryServiceConfigW(
        service,
        buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW,
        bytes_needed,
        &mut bytes_needed,
    );
    CloseServiceHandle(service);

    if res == 0 {
        return (None, "Unknown".to_string());
    }

    let config = &*(buf.as_ptr() as *const QUERY_SERVICE_CONFIGW);
    let path = wide_ptr_to_string(config.lpBinaryPathName);
    let startup_type = match config.dwStartType {
        2 => "Automatic",
        3 => "Manual",
        4 => "Disabled",
        _ => "Unknown",
    }.to_string();

    let clean_path = if !path.is_empty() {
        let trimmed = path.trim_matches('"').to_string();
        Some(trimmed)
    } else {
        None
    };

    (clean_path, startup_type)
}
