use serde::{Deserialize, Serialize};

/// Fixed WFP Provider GUID for BandwidthX
/// {87B4699E-5BDF-4CB8-9B88-14F77C862BC1}
pub const BANDWIDTHX_PROVIDER_GUID: &str = "87b4699e-5bdf-4cb8-9b88-14f77c862bc1";

/// Fixed WFP Sublayer GUID for BandwidthX
/// {D0B27163-95D4-4A47-A7EB-29DF34220AA5}
pub const BANDWIDTHX_SUBLAYER_GUID: &str = "d0b27163-95d4-4a47-a7eb-29df34220aa5";

/// Named Pipe address for local secure IPC between UI backend and privileged Windows Service
pub const BANDWIDTHX_IPC_PIPE_NAME: &str = r"\\.\pipe\BandwidthX_IPC";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Allow,
    Block,
    DownloadLimit,
    UploadLimit,
    BidirectionalLimit,
    Priority,
    ExclusiveExemption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Inbound,
    Outbound,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkRule {
    pub id: String,
    pub app_name: String,
    pub executable_path: String,
    pub rule_type: RuleType,
    pub direction: Direction,
    pub limit_bytes_per_sec: Option<u64>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExclusiveModeState {
    pub is_active: bool,
    pub target_pid: Option<u32>,
    pub target_app_name: Option<String>,
    pub target_exe_path: Option<String>,
    pub allowed_exemptions: Vec<String>,
    pub restricted_apps_count: usize,
    pub allowed_apps_count: usize,
    pub activated_at: Option<String>,
    pub last_error: Option<String>,
}

impl Default for ExclusiveModeState {
    fn default() -> Self {
        Self {
            is_active: false,
            target_pid: None,
            target_app_name: None,
            target_exe_path: None,
            allowed_exemptions: vec![
                "svchost.exe".to_string(),
                "system".to_string(),
                "lsass.exe".to_string(),
                "services.exe".to_string(),
            ],
            restricted_apps_count: 0,
            allowed_apps_count: 0,
            activated_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessNetworkInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
    pub download_bytes_sec: u64,
    pub upload_bytes_sec: u64,
    pub total_download_bytes: u64,
    pub total_upload_bytes: u64,
    pub connection_count: usize,
    pub is_exclusive_target: bool,
    pub is_blocked: bool,
    pub has_rules: bool,
    pub active_tcp_connections: usize,
    pub active_udp_endpoints: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalNetworkStats {
    pub download_speed_bps: u64,
    pub upload_speed_bps: u64,
    pub total_download_bytes: u64,
    pub total_upload_bytes: u64,
    pub active_connections: usize,
    pub detected_applications: usize,
    pub adapter_name: String,
    pub adapter_type: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub is_running: bool,
    pub is_elevated: bool,
    pub is_service_connected: bool,
    pub exclusive_mode_active: bool,
    pub active_filter_count: u32,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum IpcRequest {
    Ping,
    GetStatus,
    EnableExclusiveMode {
        target_exe_path: String,
        target_app_name: String,
        exemptions: Vec<String>,
    },
    DisableExclusiveMode,
    ApplyRule {
        rule: NetworkRule,
    },
    RemoveRule {
        rule_id: String,
        executable_path: String,
    },
    EmergencyRestore,
    GetActiveFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum IpcResponse {
    Ok { message: String },
    Pong,
    Status(ServiceStatus),
    FilterCount { count: u32 },
    Error { code: u32, message: String },
}
