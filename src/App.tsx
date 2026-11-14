import React, { useState, useEffect, useRef } from 'react';
import { 
  PageType, 
  GlobalNetworkStats, 
  ProcessNetworkInfo, 
  ExclusiveModeState, 
  ServiceStatus, 
  NetworkRule, 
  AnalyticsRecord, 
  AppSettings,
  TrafficHistoryPoint 
} from './types';
import { api } from './services/api';
import { Header } from './components/layout/Header';
import { Sidebar } from './components/layout/Sidebar';
import { StatusBar } from './components/layout/StatusBar';
import { OverviewPage } from './components/dashboard/OverviewPage';
import { ApplicationsPage } from './components/applications/ApplicationsPage';
import { ExclusiveModePage } from './components/exclusive/ExclusiveModePage';
import { AnalyticsPage } from './components/analytics/AnalyticsPage';
import { RulesPage } from './components/rules/RulesPage';
import { SettingsPage } from './components/settings/SettingsPage';

export function App() {
  const [currentPage, setCurrentPage] = useState<PageType>('overview');
  const [isMonitoring, setIsMonitoring] = useState(true);
  const [isGraphPaused, setIsGraphPaused] = useState(false);
  const [notification, setNotification] = useState<string | null>(null);

  // Core State
  const [stats, setStats] = useState<GlobalNetworkStats>({
    download_speed_bps: 0,
    upload_speed_bps: 0,
    total_download_bytes: 0,
    total_upload_bytes: 0,
    active_connections: 0,
    detected_applications: 0,
    adapter_name: 'Detecting...',
    adapter_type: 'Unknown',
    is_connected: true,
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
    is_running: true,
    is_elevated: true,
    is_service_connected: true,
    exclusive_mode_active: false,
    active_filter_count: 0,
    version: '1.0.0',
  });

  const [rules, setRules] = useState<NetworkRule[]>([]);
  const [analytics, setAnalytics] = useState<AnalyticsRecord[]>([]);
  const [settings, setSettings] = useState<AppSettings>({
    start_with_windows: false,
    start_minimized: false,
    monitoring_interval_ms: 1000,
    data_retention_days: 30,
    selected_adapter: 'Auto-detect active',
    notifications_enabled: true,
    theme: 'charcoal-dark',
    fail_safe_timeout_seconds: 30,
  });

  // 60-second rolling history
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

      // Append to graph history if not paused
      if (!isGraphPaused) {
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
      }
    } catch (err) {
      console.error('Error fetching telemetry:', err);
    }
  };

  // Polling loop
  useEffect(() => {
    refreshTelemetry();
    const interval = setInterval(() => {
      if (isMonitoring) {
        refreshTelemetry();
      }
    }, settings.monitoring_interval_ms || 1000);

    return () => clearInterval(interval);
  }, [isMonitoring, isGraphPaused, settings.monitoring_interval_ms]);

  // Load analytics and settings once
  useEffect(() => {
    api.getAnalytics('hourly').then(setAnalytics).catch(console.error);
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
  const handleToggleExclusive = async (targetPid?: number, exePath?: string, appName?: string) => {
    try {
      if (exclusiveState.is_active) {
        const res = await api.disableExclusiveMode();
        setExclusiveState(res);
        showNotification('Exclusive Mode disabled. Normal network access restored.');
      } else if (targetPid && exePath && appName) {
        const res = await api.enableExclusiveMode(targetPid, exePath, appName);
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

  // Action: Save Rule
  const handleSaveRule = async (rule: NetworkRule) => {
    try {
      await api.saveRule(rule);
      showNotification(`Saved policy for ${rule.app_name}`);
      refreshTelemetry();
    } catch (err: any) {
      showNotification(`Failed to save rule: ${err?.message}`);
    }
  };

  // Action: Delete Rule
  const handleDeleteRule = async (ruleId: string, exePath: string) => {
    try {
      await api.deleteRule(ruleId, exePath);
      showNotification('Rule deleted successfully');
      refreshTelemetry();
    } catch (err: any) {
      showNotification(`Failed to delete rule: ${err?.message}`);
    }
  };

  // Action: Export CSV
  const handleExportCsv = async (range: string) => {
    try {
      const csv = await api.exportAnalyticsCsv(range);
      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.setAttribute('href', url);
      link.setAttribute('download', `bandwidthx_telemetry_${range}_${Date.now()}.csv`);
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      showNotification('Telemetry report exported to CSV successfully.');
    } catch (err: any) {
      showNotification(`Export error: ${err?.message}`);
    }
  };

  return (
    <div className="flex flex-col h-screen w-screen bg-[#0B0F10] text-[#EDF5F0] overflow-hidden font-sans">
      {/* Toast Notification */}
      {notification && (
        <div className="fixed top-4 right-4 z-50 bg-[#12191A] border border-[#91DB79]/50 text-[#EDF5F0] px-4 py-2.5 rounded-xl shadow-2xl text-xs font-medium flex items-center space-x-2 animate-bounce">
          <span className="w-2 h-2 rounded-full bg-[#91DB79]"></span>
          <span>{notification}</span>
        </div>
      )}

      {/* Top Header */}
      <Header
        exclusiveState={exclusiveState}
        serviceStatus={serviceStatus}
        onEmergencyRestore={handleEmergencyRestore}
        onRefresh={refreshTelemetry}
      />

      {/* Main Workspace Body */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Navigation Sidebar */}
        <Sidebar
          currentPage={currentPage}
          onNavigate={setCurrentPage}
          exclusiveActive={exclusiveState.is_active}
          activeRulesCount={rules.length}
        />

        {/* Scrollable Page Viewport */}
        <main className="flex-1 overflow-y-auto p-6 bg-[#0B0F10]">
          {currentPage === 'overview' && (
            <OverviewPage
              stats={stats}
              processes={processes}
              history={history}
              isGraphPaused={isGraphPaused}
              onToggleGraphPause={() => setIsGraphPaused(!isGraphPaused)}
              onSelectExclusive={(proc) => {
                setCurrentPage('exclusive');
                handleToggleExclusive(proc.pid, proc.exe_path, proc.name);
              }}
              onNavigateToApplications={() => setCurrentPage('applications')}
            />
          )}

          {currentPage === 'applications' && (
            <ApplicationsPage
              processes={processes}
              rules={rules}
              onToggleBlock={handleToggleBlock}
              onSelectExclusive={(proc) => {
                setCurrentPage('exclusive');
              }}
              onSetLimit={(proc) => {
                setCurrentPage('rules');
              }}
              onRefresh={refreshTelemetry}
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

          {currentPage === 'analytics' && (
            <AnalyticsPage
              analytics={analytics}
              onExportCsv={handleExportCsv}
            />
          )}

          {currentPage === 'rules' && (
            <RulesPage
              rules={rules}
              onSaveRule={handleSaveRule}
              onDeleteRule={handleDeleteRule}
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
                showNotification('Settings saved.');
              }}
              onEmergencyRestore={handleEmergencyRestore}
              onResetDatabase={() => {
                showNotification('Database reset.');
              }}
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
