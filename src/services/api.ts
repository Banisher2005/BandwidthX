import { 
  GlobalNetworkStats, 
  ProcessNetworkInfo, 
  ExclusiveModeState, 
  ServiceStatus, 
  NetworkRule, 
  AppSettings, 
  AnalyticsRecord 
} from '../types';

let isTauri = false;
try {
  // @ts-ignore
  isTauri = typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);
} catch {
  isTauri = false;
}

// Dynamic import or invoke wrapper
async function invokeBackend<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<T>(cmd, args);
    } catch (err) {
      console.error(`Tauri invoke failed for ${cmd}:`, err);
      throw err;
    }
  }

  // Fallback for browser preview / testing
  console.warn(`Browser fallback for command: ${cmd}`);
  return getFallbackData<T>(cmd, args);
}

export const api = {
  getGlobalNetworkStats: () => invokeBackend<GlobalNetworkStats>('get_global_network_stats'),
  getProcessNetworkList: () => invokeBackend<ProcessNetworkInfo[]>('get_process_network_list'),
  getExclusiveModeState: () => invokeBackend<ExclusiveModeState>('get_exclusive_mode_state'),
  enableExclusiveMode: (target_pid: number, exe_path: string, app_name: string) => 
    invokeBackend<ExclusiveModeState>('enable_exclusive_mode', { target_pid, exe_path, app_name }),
  disableExclusiveMode: () => invokeBackend<ExclusiveModeState>('disable_exclusive_mode'),
  getRules: () => invokeBackend<NetworkRule[]>('get_rules'),
  saveRule: (rule: NetworkRule) => invokeBackend<NetworkRule>('save_rule', { rule }),
  deleteRule: (rule_id: string, exe_path: string) => invokeBackend<boolean>('delete_rule', { rule_id, exe_path }),
  emergencyRestore: () => invokeBackend<boolean>('emergency_restore'),
  getServiceStatus: () => invokeBackend<ServiceStatus>('get_service_status'),
  getAnalytics: (range: string) => invokeBackend<AnalyticsRecord[]>('get_analytics', { range }),
  exportAnalyticsCsv: (range: string) => invokeBackend<string>('export_analytics_csv', { range }),
  getSettings: () => invokeBackend<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => invokeBackend<boolean>('save_settings', { settings }),
};

// Fallback provider for interactive browser-only preview testing
function getFallbackData<T>(cmd: string, _args?: Record<string, unknown>): T {
  if (cmd === 'get_global_network_stats') {
    return {
      download_speed_bps: 4250000,
      upload_speed_bps: 1120000,
      total_download_bytes: 5320000000,
      total_upload_bytes: 1420000000,
      active_connections: 48,
      detected_applications: 14,
      adapter_name: 'Ethernet (Realtek PCIe GbE)',
      adapter_type: 'Ethernet 802.3',
      is_connected: true,
    } as unknown as T;
  }
  if (cmd === 'get_process_network_list') {
    return [
      {
        pid: 14208,
        name: 'chrome.exe',
        exe_path: 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
        download_bytes_sec: 2840000,
        upload_bytes_sec: 145000,
        total_download_bytes: 3120000000,
        total_upload_bytes: 290000000,
        connection_count: 24,
        is_exclusive_target: false,
        is_blocked: false,
        has_rules: false,
        active_tcp_connections: 20,
        active_udp_endpoints: 4,
      },
      {
        pid: 8940,
        name: 'Discord.exe',
        exe_path: 'C:\\Users\\User\\AppData\\Local\\Discord\\app-1.0.9008\\Discord.exe',
        download_bytes_sec: 420000,
        upload_bytes_sec: 380000,
        total_download_bytes: 840000000,
        total_upload_bytes: 410000000,
        connection_count: 8,
        is_exclusive_target: false,
        is_blocked: false,
        has_rules: false,
        active_tcp_connections: 6,
        active_udp_endpoints: 2,
      },
      {
        pid: 1204,
        name: 'steam.exe',
        exe_path: 'C:\\Program Files (x86)\\Steam\\steam.exe',
        download_bytes_sec: 84000,
        upload_bytes_sec: 12000,
        total_download_bytes: 650000000,
        total_upload_bytes: 52000000,
        connection_count: 5,
        is_exclusive_target: false,
        is_blocked: false,
        has_rules: false,
        active_tcp_connections: 4,
        active_udp_endpoints: 1,
      },
      {
        pid: 2440,
        name: 'spotify.exe',
        exe_path: 'C:\\Users\\User\\AppData\\Roaming\\Spotify\\Spotify.exe',
        download_bytes_sec: 320000,
        upload_bytes_sec: 18000,
        total_download_bytes: 420000000,
        total_upload_bytes: 14000000,
        connection_count: 6,
        is_exclusive_target: false,
        is_blocked: false,
        has_rules: false,
        active_tcp_connections: 6,
        active_udp_endpoints: 0,
      }
    ] as unknown as T;
  }
  if (cmd === 'get_exclusive_mode_state') {
    return {
      is_active: false,
      target_pid: null,
      target_app_name: null,
      target_exe_path: null,
      allowed_exemptions: ['svchost.exe', 'System', 'lsass.exe', 'services.exe'],
      restricted_apps_count: 0,
      allowed_apps_count: 4,
      activated_at: null,
      last_error: null,
    } as unknown as T;
  }
  if (cmd === 'get_service_status') {
    return {
      is_running: true,
      is_elevated: true,
      is_service_connected: true,
      exclusive_mode_active: false,
      active_filter_count: 0,
      version: '1.0.0',
    } as unknown as T;
  }
  if (cmd === 'get_rules') {
    return [] as unknown as T;
  }
  if (cmd === 'get_settings') {
    return {
      start_with_windows: false,
      start_minimized: false,
      monitoring_interval_ms: 1000,
      data_retention_days: 30,
      selected_adapter: 'Auto-detect active',
      notifications_enabled: true,
      theme: 'charcoal-dark',
      fail_safe_timeout_seconds: 30,
    } as unknown as T;
  }
  return {} as unknown as T;
}
