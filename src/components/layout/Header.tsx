import React from 'react';
import { 
  Wifi, 
  ShieldCheck, 
  AlertTriangle, 
  RotateCcw, 
  Settings,
  CircleDot
} from 'lucide-react';
import { ExclusiveModeState, ServiceStatus, GlobalNetworkStats, PageType } from '../../types';

interface HeaderProps {
  stats: GlobalNetworkStats;
  exclusiveState: ExclusiveModeState;
  serviceStatus: ServiceStatus;
  onEmergencyRestore: () => void;
  onRefresh: () => void;
  onNavigate: (page: PageType) => void;
}

export const Header: React.FC<HeaderProps> = ({
  stats,
  exclusiveState,
  serviceStatus,
  onEmergencyRestore,
  onRefresh,
  onNavigate,
}) => {
  return (
    <header className="h-16 border-b border-glass-border bg-background/50 backdrop-blur-md px-6 flex items-center justify-between z-10 select-none shrink-0">
      {/* Left: Active Interface & Adapter Info */}
      <div className="flex items-center space-x-3">
        <div className="flex items-center space-x-2 px-3 py-1.5 rounded-xl glass-card text-xs">
          <Wifi className="w-3.5 h-3.5 text-accent-secondary" />
          <span className="font-medium text-content-primary truncate max-w-[200px]">
            {stats.adapter_name || 'Detecting interface...'}
          </span>
          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-white/[0.06] text-content-secondary">
            {stats.adapter_type || 'Active'}
          </span>
        </div>

        {/* Exclusive mode active alert */}
        {exclusiveState.is_active && (
          <div className="flex items-center space-x-2 px-3 py-1 rounded-xl bg-status-danger/15 border border-status-danger/40 text-status-danger text-xs font-semibold animate-pulse">
            <span className="w-2 h-2 rounded-full bg-status-danger"></span>
            <span>EXCLUSIVE: {exclusiveState.target_app_name}</span>
          </div>
        )}
      </div>

      {/* Right: Service status & Quick Actions */}
      <div className="flex items-center space-x-2.5">
        {/* Service connection badge */}
        <div className="flex items-center space-x-2 px-3 py-1.5 rounded-xl glass-card text-xs">
          <CircleDot className={`w-3 h-3 ${serviceStatus.is_running ? 'text-accent-primary animate-pulse' : 'text-status-danger'}`} />
          <span className="text-content-secondary text-[11px] font-medium">
            {serviceStatus.is_running ? 'WFP Service Running' : 'Service Offline'}
          </span>
          {serviceStatus.is_elevated && (
            <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-accent-primary/10 text-accent-primary border border-accent-primary/20 flex items-center gap-1">
              <ShieldCheck className="w-3 h-3" />
              ELEVATED
            </span>
          )}
        </div>

        {/* Manual refresh button */}
        <button
          onClick={onRefresh}
          className="p-2 rounded-xl glass-card text-content-secondary hover:text-content-primary glass-card-hover transition-colors"
          title="Refresh Network Telemetry"
        >
          <RotateCcw className="w-4 h-4" />
        </button>

        {/* Emergency Restore Button */}
        <button
          onClick={onEmergencyRestore}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-status-danger/15 hover:bg-status-danger/25 border border-status-danger/30 text-status-danger text-xs font-medium transition-all"
          title="Emergency rollback: Instantly removes all BandwidthX WFP filters and restores normal networking"
        >
          <AlertTriangle className="w-3.5 h-3.5" />
          <span className="hidden sm:inline">Emergency Restore</span>
        </button>

        {/* Settings Shortcut */}
        <button
          onClick={() => onNavigate('settings')}
          className="p-2 rounded-xl glass-card text-content-secondary hover:text-content-primary glass-card-hover transition-colors"
          title="Settings"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
