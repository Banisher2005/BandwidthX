export type PageType = 'overview' | 'applications' | 'exclusive' | 'analytics' | 'rules' | 'settings';

export type RuleType = 
  | 'allow' 
  | 'block' 
  | 'download_limit' 
  | 'upload_limit' 
  | 'bidirectional_limit' 
  | 'priority' 
  | 'exclusive_exemption';

export type Direction = 'inbound' | 'outbound' | 'both';

export interface NetworkRule {
  id: string;
  app_name: string;
  executable_path: string;
  rule_type: RuleType;
  direction: Direction;
  limit_bytes_per_sec?: number | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProcessNetworkInfo {
  pid: number;
  name: string;
  exe_path: string;
  download_bytes_sec: number;
  upload_bytes_sec: number;
  total_download_bytes: number;
  total_upload_bytes: number;
  connection_count: number;
  is_exclusive_target: boolean;
  is_blocked: boolean;
  has_rules: boolean;
  active_tcp_connections: number;
  active_udp_endpoints: number;
  icon?: string;
}

export interface GlobalNetworkStats {
  download_speed_bps: number;
  upload_speed_bps: number;
  total_download_bytes: number;
  total_upload_bytes: number;
  active_connections: number;
  detected_applications: number;
  adapter_name: string;
  adapter_type: string;
  is_connected: boolean;
}

export interface ExclusiveModeState {
  is_active: boolean;
  target_pid: number | null;
  target_app_name: string | null;
  target_exe_path: string | null;
  allowed_exemptions: string[];
  restricted_apps_count: number;
  allowed_apps_count: number;
  activated_at: string | null;
  last_error: string | null;
}

export interface ServiceStatus {
  is_running: boolean;
  is_elevated: boolean;
  is_service_connected: boolean;
  exclusive_mode_active: boolean;
  active_filter_count: number;
  version: string;
}

export interface TrafficHistoryPoint {
  time: string;
  download: number; // in KB/s or MB/s
  upload: number;
  rawDownload: number;
  rawUpload: number;
}

export interface AnalyticsRecord {
  id?: number;
  app_name: string;
  executable_path: string;
  bytes_downloaded: number;
  bytes_uploaded: number;
  timestamp: string;
}

export interface AppSettings {
  start_with_windows: boolean;
  start_minimized: boolean;
  monitoring_interval_ms: number;
  data_retention_days: number;
  selected_adapter: string;
  notifications_enabled: boolean;
  theme: string;
  fail_safe_timeout_seconds: number;
}
