import React from 'react';
import { 
  Settings, 
  ShieldCheck, 
  AlertTriangle, 
  RotateCcw, 
  Cpu, 
  HardDrive, 
  Bell, 
  Monitor,
  Check
} from 'lucide-react';
import { AppSettings, ServiceStatus } from '../../types';

interface SettingsPageProps {
  settings: AppSettings;
  serviceStatus: ServiceStatus;
  onUpdateSettings: (settings: AppSettings) => void;
  onEmergencyRestore: () => void;
  onResetDatabase: () => void;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({
  settings,
  serviceStatus,
  onUpdateSettings,
  onEmergencyRestore,
  onResetDatabase,
}) => {
  return (
    <div className="space-y-6 max-w-4xl mx-auto">
      {/* 1. Header */}
      <div>
        <h2 className="text-xl font-bold text-[#EDF5F0] flex items-center gap-2">
          <Settings className="w-5 h-5 text-[#91DB79]" />
          Application Settings & Diagnostics
        </h2>
        <p className="text-xs text-[#91A39A] mt-1">
          Configure telemetry frequency, startup preferences, and system recovery mechanisms
        </p>
      </div>

      {/* 2. System Service & Privilege Status */}
      <div className="p-5 rounded-xl bg-[#12191A] border border-[#192223] space-y-4">
        <h3 className="text-sm font-semibold text-[#EDF5F0] flex items-center gap-2">
          <Cpu className="w-4 h-4 text-[#57C7B5]" />
          Windows Filtering Platform Diagnostics
        </h3>

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs">
          <div className="p-3 rounded-lg bg-[#0B0F10] border border-[#192223]">
            <div className="text-[#91A39A] mb-1">Process Privilege Level</div>
            <div className="font-mono font-bold text-sm text-[#91DB79] flex items-center gap-1.5">
              <ShieldCheck className="w-4 h-4" />
              {serviceStatus.is_elevated ? 'Administrator (Full WFP)' : 'Standard User'}
            </div>
          </div>

          <div className="p-3 rounded-lg bg-[#0B0F10] border border-[#192223]">
            <div className="text-[#91A39A] mb-1">WFP Engine State</div>
            <div className="font-mono font-bold text-sm text-[#57C7B5]">
              {serviceStatus.is_running ? 'Online & Bound' : 'Offline'}
            </div>
          </div>

          <div className="p-3 rounded-lg bg-[#0B0F10] border border-[#192223]">
            <div className="text-[#91A39A] mb-1">Active BandwidthX Filters</div>
            <div className="font-mono font-bold text-sm text-[#EDF5F0]">
              {serviceStatus.active_filter_count} Filters in ALE
            </div>
          </div>
        </div>
      </div>

      {/* 3. General Preferences */}
      <div className="p-5 rounded-xl bg-[#12191A] border border-[#192223] space-y-4">
        <h3 className="text-sm font-semibold text-[#EDF5F0] flex items-center gap-2">
          <Monitor className="w-4 h-4 text-[#91DB79]" />
          General Preferences
        </h3>

        <div className="space-y-3 divide-y divide-[#192223]/60 text-xs">
          {/* Start with Windows */}
          <div className="pt-3 first:pt-0 flex items-center justify-between">
            <div>
              <div className="font-medium text-[#EDF5F0]">Start with Windows</div>
              <div className="text-[#91A39A]">Launch BandwidthX background helper when logging in</div>
            </div>
            <input
              type="checkbox"
              checked={settings.start_with_windows}
              onChange={(e) => onUpdateSettings({ ...settings, start_with_windows: e.target.checked })}
              className="w-4 h-4 rounded border-[#192223] accent-[#91DB79]"
            />
          </div>

          {/* Start Minimized */}
          <div className="pt-3 flex items-center justify-between">
            <div>
              <div className="font-medium text-[#EDF5F0]">Start Minimized to Tray</div>
              <div className="text-[#91A39A]">Run quietly in the Windows system notification area on startup</div>
            </div>
            <input
              type="checkbox"
              checked={settings.start_minimized}
              onChange={(e) => onUpdateSettings({ ...settings, start_minimized: e.target.checked })}
              className="w-4 h-4 rounded border-[#192223] accent-[#91DB79]"
            />
          </div>

          {/* Telemetry Interval */}
          <div className="pt-3 flex items-center justify-between">
            <div>
              <div className="font-medium text-[#EDF5F0]">Monitoring Refresh Interval</div>
              <div className="text-[#91A39A]">Speed calculation frequency from IP Helper counters</div>
            </div>
            <select
              value={settings.monitoring_interval_ms}
              onChange={(e) => onUpdateSettings({ ...settings, monitoring_interval_ms: parseInt(e.target.value, 10) })}
              className="px-3 py-1.5 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
            >
              <option value="500">500 ms (High Precision)</option>
              <option value="1000">1000 ms (Recommended)</option>
              <option value="2000">2000 ms (Power Saver)</option>
            </select>
          </div>

          {/* Data Retention */}
          <div className="pt-3 flex items-center justify-between">
            <div>
              <div className="font-medium text-[#EDF5F0]">Analytics Data Retention</div>
              <div className="text-[#91A39A]">Maximum duration to keep per-application historical records in SQLite</div>
            </div>
            <select
              value={settings.data_retention_days}
              onChange={(e) => onUpdateSettings({ ...settings, data_retention_days: parseInt(e.target.value, 10) })}
              className="px-3 py-1.5 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
            >
              <option value="7">7 Days</option>
              <option value="30">30 Days</option>
              <option value="90">90 Days</option>
              <option value="365">1 Year</option>
            </select>
          </div>
        </div>
      </div>

      {/* 4. Prominent Emergency Restore & Fail-Safe Box */}
      <div className="p-5 rounded-xl bg-[#12191A] border border-[#EF6B70]/40 space-y-4">
        <div className="flex items-start space-x-3">
          <AlertTriangle className="w-5 h-5 text-[#EF6B70] shrink-0 mt-0.5" />
          <div className="space-y-1">
            <h3 className="text-sm font-bold text-[#EF6B70]">
              Emergency Network Recovery & Rollback
            </h3>
            <p className="text-xs text-[#91A39A] leading-relaxed">
              If an unexpected connection issue occurs or you need to instantly restore unrestricted default Windows networking, this button immediately purges all BandwidthX WFP sublayers, filters, and ALE rules.
            </p>
          </div>
        </div>

        <div className="pt-2 flex flex-col sm:flex-row items-center justify-between gap-3 border-t border-[#192223]">
          <span className="text-xs font-mono text-[#91A39A]">
            Safe fail-safe: Will not affect default Windows Firewall rules.
          </span>
          <button
            onClick={onEmergencyRestore}
            className="w-full sm:w-auto px-5 py-2.5 rounded-lg bg-[#EF6B70] hover:bg-[#EF6B70]/90 text-[#0B0F10] font-bold text-xs transition-colors flex items-center justify-center space-x-2 shadow-lg shadow-[#EF6B70]/20"
          >
            <RotateCcw className="w-4 h-4" />
            <span>Restore Normal Network Mode</span>
          </button>
        </div>
      </div>
    </div>
  );
};
