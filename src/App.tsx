import React, { useState, useEffect } from 'react';
import { 
  PageType, 
  GlobalNetworkStats, 
  ProcessNetworkInfo, 
  ExclusiveModeState, 
  ServiceStatus, 
  NetworkRule, 
  AppSettings,
  TrafficHistoryPoint 
} from './types';
import { api } from './services/api';
import { Header } from './components/layout/Header';
import { Sidebar } from './components/layout/Sidebar';
import { StatusBar } from './components/layout/StatusBar';
import { NetworkPage } from './components/dashboard/NetworkPage';
import { ApplicationsPage } from './components/applications/ApplicationsPage';
import { WindowsServicesPage } from './components/services/WindowsServicesPage';
import { ExclusiveModePage } from './components/exclusive/ExclusiveModePage';
import { SettingsPage } from './components/settings/SettingsPage';

export function App() {
  const [currentPage, setCurrentPage] = useState<PageType>('network');
  const [isMonitoring] = useState(true);
  const [notification, setNotification] = useState<string | null>(null);

  // Core Global Telemetry State
  const [stats, setStats] = useState<GlobalNetworkStats>({
    download_speed_bps: 0,
    upload_speed_bps: 0,
    total_download_bytes: 0,
    total_upload_bytes: 0,
    active_connections: 0,
    detected_applications: 0,
    adapter_name: 'Detecting network adapter...',
    adapter_type: 'Unknown',
    is_connected: false,
  });

  const [processes, setProcesses] = useState<ProcessNetworkInfo[]>([]);
  const [exclusiveState, setExclusiveState] = useState<ExclusiveModeState>({
    is_active: false,
    target_pid: null,
    target_app_name: null,
    target_exe_path: null,
    allowed_exemptions: ['svchost.exe', 'System', 'lsass.exe', 'services.exe'],
    restricted_apps_count: 0,
    allowed_apps_count: 0,
    activated_at: null,
    last_error: null,
  });

  const [serviceStatus, setServiceStatus] = useState<ServiceStatus>({
    is_running: false,
    is_elevated: false,
    is_service_connected: false,
    exclusive_mode_active: false,
    active_filter_count: 0,
    version: '3.0.0',
  });

  const [rules, setRules] = useState<NetworkRule[]>([]);
  const [settings, setSettings] = useState<AppSettings>({
    start_with_windows: false,
    start_minimized: false,
    monitoring_interval_ms: 1000,
    speed_test_endpoint: 'https://speed.cloudflare.com',
    data_retention_days: 30,
    selected_adapter: 'Auto-detect active',
    notifications_enabled: true,
    theme: 'charcoal-dark',
    fail_safe_timeout_seconds: 30,
  });

  // 60-second rolling history buffer
  const [history, setHistory] = useState<TrafficHistoryPoint[]>(() => {
    const initial: TrafficHistoryPoint[] = [];
    const now = new Date();
    for (let i = 59; i >= 0; i--) {
      const t = new Date(now.getTime() - i * 1000);
      initial.push({
        time: t.toTimeString().split(' ')[0],
        download: 0,
        upload: 0,
        rawDownload: 0,
        rawUpload: 0,
      });
    }
    return initial;
  });

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => setNotification(null), 4000);
  };

  // Telemetry fetcher
  const refreshTelemetry = async () => {
    try {
      const [newStats, newProcs, newExclusive, newStatus, newRules] = await Promise.all([
        api.getGlobalNetworkStats(),
        api.getProcessNetworkList(),
        api.getExclusiveModeState(),
        api.getServiceStatus(),
        api.getRules(),
      ]);

      setStats(newStats);
      setProcesses(newProcs);
      setExclusiveState(newExclusive);
      setServiceStatus(newStatus);
      setRules(newRules);

      // Append to rolling history
      const timeStr = new Date().toTimeString().split(' ')[0];
      setHistory((prev) => {
        const next = [
          ...prev.slice(1),
          {
            time: timeStr,
            download: newStats.download_speed_bps,
            upload: newStats.upload_speed_bps,
            rawDownload: newStats.download_speed_bps,
            rawUpload: newStats.upload_speed_bps,
          },
        ];
        return next;
      });
    } catch (err) {
      console.error('Error fetching telemetry:', err);
    }
  };

  // Continuous polling loop
  useEffect(() => {
    refreshTelemetry();
    const interval = setInterval(() => {
      if (isMonitoring) {
        refreshTelemetry();
      }
    }, settings.monitoring_interval_ms || 1000);

    return () => clearInterval(interval);
  }, [isMonitoring, settings.monitoring_interval_ms]);

  // Load settings on initial mount
  useEffect(() => {
    api.getSettings().then(setSettings).catch(console.error);
  }, []);

  // Action: Toggle Block application
  const handleToggleBlock = async (proc: ProcessNetworkInfo) => {
    try {
      if (proc.is_blocked) {
        await api.deleteRule(`block-${proc.name}`, proc.exe_path);
        showNotification(`Unblocked network access for ${proc.name}`);
      } else {
        const rule: NetworkRule = {
          id: `block-${Date.now()}`,
          app_name: proc.name,
          executable_path: proc.exe_path,
          rule_type: 'block',
          direction: 'both',
          limit_bytes_per_sec: null,
          enabled: true,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        };
        await api.saveRule(rule);
        showNotification(`Blocked network access for ${proc.name}`);
      }
      refreshTelemetry();
    } catch (err: any) {
      showNotification(`Enforcement error: ${err?.message || 'Operation failed'}`);
    }
  };

  // Action: Toggle Exclusive Mode
  const handleToggleExclusive = async (targetPid?: number, exePath?: string, appName?: string, creationTime?: number) => {
    try {
      if (exclusiveState.is_active) {
        const res = await api.disableExclusiveMode();
        setExclusiveState(res);
        showNotification('Exclusive Mode disabled. Normal network access restored.');
      } else if (targetPid && exePath && appName) {
        const res = await api.enableExclusiveMode(targetPid, exePath, appName, creationTime);
        setExclusiveState(res);
        showNotification(`Exclusive Mode active for ${appName}! Other apps restricted.`);
      }
      refreshTelemetry();
    } catch (err: any) {
      showNotification(`Exclusive Mode error: ${err?.message || 'Failed to toggle mode'}`);
    }
  };

  // Action: Emergency Restore
  const handleEmergencyRestore = async () => {
    try {
      await api.emergencyRestore();
      showNotification('EMERGENCY RESTORE: All BandwidthX filters purged. Normal network active.');
      refreshTelemetry();
    } catch (err: any) {
      showNotification(`Restore error: ${err?.message || 'Failed to restore'}`);
    }
  };

  // Blocked applications count
  const blockedCount = processes.filter((p) => p.is_blocked).length;

  return (
    <div className="flex flex-col h-screen w-screen bg-background text-content-primary overflow-hidden font-sans">
      {/* Toast Notification */}
      {notification && (
        <div className="fixed top-4 right-4 z-50 glass-panel-elevated text-content-primary px-4 py-2.5 rounded-xl shadow-2xl text-xs font-medium flex items-center space-x-2 border border-accent-primary/50 animate-bounce">
          <span className="w-2 h-2 rounded-full bg-accent-primary"></span>
          <span>{notification}</span>
        </div>
      )}

      {/* Top Header */}
      <Header
        stats={stats}
        exclusiveState={exclusiveState}
        serviceStatus={serviceStatus}
        onEmergencyRestore={handleEmergencyRestore}
        onRefresh={refreshTelemetry}
        onNavigate={setCurrentPage}
      />

      {/* Main Workspace Body */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Navigation Sidebar */}
        <Sidebar
          currentPage={currentPage}
          onNavigate={setCurrentPage}
          exclusiveActive={exclusiveState.is_active}
          blockedCount={blockedCount}
        />

        {/* Scrollable Page Viewport */}
        <main className="flex-1 overflow-y-auto p-6 bg-background">
          {(currentPage === 'network' || currentPage === 'overview' || currentPage === 'analytics') && (
            <NetworkPage
              stats={stats}
              history={history}
              speedTestEndpoint={settings.speed_test_endpoint}
            />
          )}

          {(currentPage === 'applications' || currentPage === 'rules') && (
            <ApplicationsPage
              processes={processes}
              rules={rules}
              onToggleBlock={handleToggleBlock}
              onSelectExclusive={(_proc) => {
                setCurrentPage('exclusive');
              }}
              onRefresh={refreshTelemetry}
              onNotification={showNotification}
            />
          )}

          {currentPage === 'services' && (
            <WindowsServicesPage
              onNotification={showNotification}
            />
          )}

          {currentPage === 'exclusive' && (
            <ExclusiveModePage
              exclusiveState={exclusiveState}
              serviceStatus={serviceStatus}
              processes={processes}
              onToggleExclusive={handleToggleExclusive}
              onEmergencyRestore={handleEmergencyRestore}
            />
          )}

          {currentPage === 'settings' && (
            <SettingsPage
              settings={settings}
              serviceStatus={serviceStatus}
              onUpdateSettings={(newSettings) => {
                setSettings(newSettings);
                api.saveSettings(newSettings);
              }}
              onEmergencyRestore={handleEmergencyRestore}
              onResetDatabase={() => {
                showNotification('Database reset.');
              }}
              onNotification={showNotification}
            />
          )}
        </main>
      </div>

      {/* Persistent Bottom Status Bar */}
      <StatusBar
        stats={stats}
        serviceStatus={serviceStatus}
        isMonitoring={isMonitoring}
      />
    </div>
  );
}

export default App;
