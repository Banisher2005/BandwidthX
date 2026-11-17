import React from 'react';
import { Wifi, WifiOff, ShieldCheck, ShieldAlert, Cpu, Activity } from 'lucide-react';
import { GlobalNetworkStats, ServiceStatus } from '../../types';

interface StatusBarProps {
  stats: GlobalNetworkStats;
  serviceStatus: ServiceStatus;
  isMonitoring: boolean;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  stats,
  serviceStatus,
  isMonitoring,
}) => {
  return (
    <footer className="h-9 bg-background/90 border-t border-glass-border px-4 flex items-center justify-between text-xs font-mono text-content-secondary select-none z-20 backdrop-blur-md">
      {/* Left side: Network Connection & Adapter */}
      <div className="flex items-center space-x-6">
        {/* Connection status */}
        <div className="flex items-center space-x-2">
          {stats.is_connected ? (
            <>
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-accent-primary opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-accent-primary"></span>
              </span>
              <span className="text-content-primary flex items-center gap-1.5">
                <Wifi className="w-3.5 h-3.5 text-accent-primary" />
                Online
              </span>
            </>
          ) : (
            <>
              <span className="w-2 h-2 rounded-full bg-status-danger"></span>
              <span className="text-status-danger flex items-center gap-1.5">
                <WifiOff className="w-3.5 h-3.5" />
                Disconnected
              </span>
            </>
          )}
        </div>

        {/* Current Active Adapter */}
        <div className="flex items-center space-x-1.5 text-content-secondary truncate max-w-[280px]">
          <span className="text-content-muted">Adapter:</span>
          <span className="text-content-primary truncate" title={stats.adapter_name}>
            {stats.adapter_name || 'Detecting interface...'}
          </span>
        </div>
      </div>

      {/* Right side: Monitoring, Service & Privilege status */}
      <div className="flex items-center space-x-6">
        {/* Monitoring status */}
        <div className="flex items-center space-x-1.5">
          <Activity className={`w-3.5 h-3.5 ${isMonitoring ? 'text-accent-primary' : 'text-status-warning'}`} />
          <span>Telemetry:</span>
          <span className={isMonitoring ? 'text-accent-primary' : 'text-status-warning'}>
            {isMonitoring ? 'Live' : 'Paused'}
          </span>
        </div>

        {/* Enforcement service status */}
        <div className="flex items-center space-x-1.5">
          <Cpu className="w-3.5 h-3.5 text-accent-secondary" />
          <span>WFP Engine:</span>
          <span className={serviceStatus.is_running ? 'text-accent-secondary' : 'text-status-danger'}>
            {serviceStatus.is_running ? 'Active' : 'Offline'}
          </span>
        </div>

        {/* Administrator privileges */}
        <div className="flex items-center space-x-1.5">
          {serviceStatus.is_elevated ? (
            <>
              <ShieldCheck className="w-3.5 h-3.5 text-accent-primary" />
              <span className="text-accent-primary">Elevated (Admin)</span>
            </>
          ) : (
            <>
              <ShieldAlert className="w-3.5 h-3.5 text-status-warning" />
              <span className="text-status-warning">Standard User</span>
            </>
          )}
        </div>
      </div>
    </footer>
  );
};