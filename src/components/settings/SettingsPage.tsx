import React, { useState } from 'react';
import { 
  Settings, 
  ShieldCheck, 
  AlertTriangle, 
  RotateCcw, 
  Cpu, 
  HardDrive, 
  Monitor,
  Activity,
  Globe,
  Save,
  CheckCircle2,
  Trash2,
  Server
} from 'lucide-react';
import { AppSettings, ServiceStatus } from '../../types';

interface SettingsPageProps {
  settings: AppSettings;
  serviceStatus: ServiceStatus;
  onUpdateSettings: (settings: AppSettings) => void;
  onEmergencyRestore: () => void;
  onResetDatabase?: () => void;
  onNotification?: (msg: string) => void;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({
  settings,
  serviceStatus,
  onUpdateSettings,
  onEmergencyRestore,
  onResetDatabase,
  onNotification,
}) => {
  const [localSettings, setLocalSettings] = useState<AppSettings>(settings);
  const [isSaved, setIsSaved] = useState(false);

  const handleSave = () => {
    onUpdateSettings(localSettings);
    setIsSaved(true);
    if (onNotification) {
      onNotification('Settings saved successfully');
    }
    setTimeout(() => setIsSaved(false), 2500);
  };

  return (
    <div className="space-y-6 max-w-4xl mx-auto">
      {/* 1. Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold text-content-primary flex items-center gap-2">
            <Settings className="w-5 h-5 text-accent-primary" />
            Application Settings & Diagnostics
          </h2>
          <p className="text-xs text-content-secondary mt-1">
            Configure telemetry frequency, speed test endpoints, and inspect kernel sublayers
          </p>
        </div>

        <button
          onClick={handleSave}
          className="px-4 py-2 rounded-xl bg-accent-primary hover:bg-accent-primary/90 text-background font-semibold text-xs transition-all flex items-center space-x-1.5 shadow-lg shadow-accent-primary/20"
        >
          {isSaved ? (
            <>
              <CheckCircle2 className="w-4 h-4" />
              <span>Saved</span>
            </>
          ) : (
            <>
              <Save className="w-4 h-4" />
              <span>Save Changes</span>
            </>
          )}
        </button>
      </div>

      {/* 2. System Service & Privilege Status */}
      <div className="glass-card p-5 rounded-2xl border border-glass-border space-y-4">
        <h3 className="text-sm font-semibold text-content-primary flex items-center gap-2">
          <Cpu className="w-4 h-4 text-accent-secondary" />
          Windows Filtering Platform Diagnostics
        </h3>

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs">
          <div className="p-3 rounded-xl bg-background/60 border border-glass-border">
            <div className="text-content-secondary mb-1">Process Privilege Level</div>
            <div className="font-mono font-bold text-sm text-accent-primary flex items-center gap-1.5">
              <ShieldCheck className="w-4 h-4" />
              {serviceStatus.is_elevated ? 'Administrator (Full WFP)' : 'Standard User'}
            </div>
          </div>

          <div className="p-3 rounded-xl bg-background/60 border border-glass-border">
            <div className="text-content-secondary mb-1">WFP Engine State</div>
            <div className="font-mono font-bold text-sm text-accent-secondary flex items-center gap-1.5">
              <Server className="w-4 h-4" />
              {serviceStatus.is_running ? 'Online & Bound' : 'Offline'}
            </div>
          </div>

          <div className="p-3 rounded-xl bg-background/60 border border-glass-border">
            <div className="text-content-secondary mb-1">Active BandwidthX Filters</div>
            <div className="font-mono font-bold text-sm text-content-primary">
              {serviceStatus.active_filter_count} Filters in ALE
            </div>
          </div>
        </div>
      </div>

      {/* 3. Speed Test & Network Preferences */}
      <div className="glass-card p-5 rounded-2xl border border-glass-border space-y-4">
        <h3 className="text-sm font-semibold text-content-primary flex items-center gap-2">
          <Globe className="w-4 h-4 text-accent-primary" />
          Speed Test & Connectivity Preferences
        </h3>

        <div className="space-y-4 text-xs">
          <div>
            <label className="block text-content-secondary font-medium mb-1.5">
              Speed Test CDN Endpoint
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={localSettings.speed_test_endpoint || 'https://speed.cloudflare.com'}
                onChange={(e) => setLocalSettings({ ...localSettings, speed_test_endpoint: e.target.value })}
                placeholder="https://speed.cloudflare.com"
                className="flex-1 px-3 py-2 rounded-xl bg-background/70 border border-glass-border text-xs text-content-primary font-mono focus:outline-none focus:border-accent-primary/50"
              />
              <button
                type="button"
                onClick={() => setLocalSettings({ ...localSettings, speed_test_endpoint: 'https://speed.cloudflare.com' })}
                className="px-3 py-2 rounded-xl bg-white/[0.04] border border-glass-border text-content-secondary hover:text-content-primary text-xs"
              >
                Reset Default
              </button>
            </div>
            <p className="text-[11px] text-content-muted mt-1">
              Supports standard HTTP latency and chunked streaming endpoints. Default: Cloudflare Edge CDN.
            </p>
          </div>
        </div>
      </div>

      {/* 4. General Preferences */}
      <div className="glass-card p-5 rounded-2xl border border-glass-border space-y-4">
        <h3 className="text-sm font-semibold text-content-primary flex items-center gap-2">
          <Monitor className="w-4 h-4 text-accent-primary" />
          General & Telemetry Preferences
        </h3>

        <div className="space-y-3 divide-y divide-glass-border text-xs">
          {/* Start with Windows */}
          <div className="pt-3 first:pt-0 flex items-center justify-between">
            <div>
              <div className="font-medium text-content-primary">Start with Windows</div>
              <div className="text-content-muted">Launch BandwidthX background helper when logging into Windows</div>
            </div>
            <input
              type="checkbox"
              checked={localSettings.start_with_windows}
              onChange={(e) => setLocalSettings({ ...localSettings, start_with_windows: e.target.checked })}
              className="w-4 h-4 rounded border-glass-border accent-accent-primary cursor-pointer"
            />
          </div>

          {/* Start Minimized */}
          <div className="pt-3 flex items-center justify-between">
            <div>
              <div className="font-medium text-content-primary">Start Minimized to System Tray</div>
              <div className="text-content-muted">Run quietly in the Windows system notification area on startup</div>
            </div>
            <input
              type="checkbox"
              checked={localSettings.start_minimized}
              onChange={(e) => setLocalSettings({ ...localSettings, start_minimized: e.target.checked })}
              className="w-4 h-4 rounded border-glass-border accent-accent-primary cursor-pointer"
            />
          </div>

          {/* Telemetry Interval */}
          <div className="pt-3 flex items-center justify-between">
            <div>
              <div className="font-medium text-content-primary">Monitoring Refresh Interval</div>
              <div className="text-content-muted">Sampling frequency for TCP EStats and IP Helper counters</div>
            </div>
            <select
              value={localSettings.monitoring_interval_ms}
              onChange={(e) => setLocalSettings({ ...localSettings, monitoring_interval_ms: parseInt(e.target.value, 10) })}
              className="px-3 py-1.5 rounded-xl bg-background/80 border border-glass-border text-xs text-content-primary focus:outline-none focus:border-accent-primary font-mono cursor-pointer"
            >
              <option value="500">500 ms (High Precision)</option>
              <option value="1000">1000 ms (Recommended)</option>
              <option value="2000">2000 ms (Power Saver)</option>
            </select>
          </div>

          {/* Data Retention */}
          <div className="pt-3 flex items-center justify-between">
            <div>
              <div className="font-medium text-content-primary">Analytics Data Retention</div>
              <div className="text-content-muted">Maximum duration to retain per-application historical records in SQLite</div>
            </div>
            <select
              value={localSettings.data_retention_days}
              onChange={(e) => setLocalSettings({ ...localSettings, data_retention_days: parseInt(e.target.value, 10) })}
              className="px-3 py-1.5 rounded-xl bg-background/80 border border-glass-border text-xs text-content-primary focus:outline-none focus:border-accent-primary font-mono cursor-pointer"
            >
              <option value="7">7 Days</option>
              <option value="30">30 Days</option>
              <option value="90">90 Days</option>
              <option value="365">1 Year</option>
            </select>
          </div>
        </div>
      </div>

      {/* 5. Prominent Emergency Restore & Fail-Safe Box */}
      <div className="glass-card p-5 rounded-2xl border border-status-danger/40 space-y-4">
        <div className="flex items-start space-x-3">
          <AlertTriangle className="w-5 h-5 text-status-danger shrink-0 mt-0.5" />
          <div className="space-y-1">
            <h3 className="text-sm font-bold text-status-danger">
              Emergency Network Recovery & Rollback
            </h3>
            <p className="text-xs text-content-secondary leading-relaxed">
              If an unexpected connection issue occurs or you need to instantly restore unrestricted default Windows networking, this button immediately purges all BandwidthX WFP sublayers, filters, and ALE rules.
            </p>
          </div>
        </div>

        <div className="pt-3 flex flex-col sm:flex-row items-center justify-between gap-3 border-t border-glass-border">
          <span className="text-xs font-mono text-content-muted">
            Safe fail-safe: Purges BandwidthX dynamic session filters without modifying system firewall rules.
          </span>
          <button
            onClick={onEmergencyRestore}
            className="w-full sm:w-auto px-5 py-2.5 rounded-xl bg-status-danger hover:bg-status-danger/90 text-background font-bold text-xs transition-colors flex items-center justify-center space-x-2 shadow-lg shadow-status-danger/20"
          >
            <RotateCcw className="w-4 h-4" />
            <span>Restore Normal Network Mode</span>
          </button>
        </div>
      </div>
    </div>
  );
};