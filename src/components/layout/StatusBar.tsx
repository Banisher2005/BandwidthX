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
    <footer className="h-9 bg-[#0B0F10] border-t border-[#192223] px-4 flex items-center justify-between text-xs font-mono text-[#91A39A] select-none z-20">
      {/* Left side: Network Connection & Adapter */}
      <div className="flex items-center space-x-6">
        {/* Connection status */}
        <div className="flex items-center space-x-2">
          {stats.is_connected ? (
            <>
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#91DB79] opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-[#91DB79]"></span>
              </span>
              <span className="text-[#EDF5F0] flex items-center gap-1.5">
                <Wifi className="w-3.5 h-3.5 text-[#91DB79]" />
                Online
              </span>
            </>
          ) : (
            <>
              <span className="w-2 h-2 rounded-full bg-[#EF6B70]"></span>
              <span className="text-[#EF6B70] flex items-center gap-1.5">
                <WifiOff className="w-3.5 h-3.5" />
                Disconnected
              </span>
            </>
          )}
        </div>

        {/* Current Active Adapter */}
        <div className="flex items-center space-x-1.5 text-[#91A39A] truncate max-w-[280px]">
          <span className="text-[#91A39A]/60">Adapter:</span>
          <span className="text-[#EDF5F0] truncate" title={stats.adapter_name}>
            {stats.adapter_name || 'Detecting interface...'}
          </span>
        </div>
      </div>

      {/* Right side: Monitoring, Service & Privilege status */}
      <div className="flex items-center space-x-6">
        {/* Monitoring status */}
        <div className="flex items-center space-x-1.5">
          <Activity className={`w-3.5 h-3.5 ${isMonitoring ? 'text-[#91DB79]' : 'text-[#E9B65C]'}`} />
          <span>Telemetry:</span>
          <span className={isMonitoring ? 'text-[#91DB79]' : 'text-[#E9B65C]'}>
            {isMonitoring ? 'Live (1000ms)' : 'Paused'}
          </span>
        </div>

        {/* Enforcement service status */}
        <div className="flex items-center space-x-1.5">
          <Cpu className="w-3.5 h-3.5 text-[#57C7B5]" />
          <span>WFP Engine:</span>
          <span className={serviceStatus.is_running ? 'text-[#57C7B5]' : 'text-[#EF6B70]'}>
            {serviceStatus.is_running ? 'Active' : 'Offline'}
          </span>
        </div>

        {/* Administrator privileges */}
        <div className="flex items-center space-x-1.5">
          {serviceStatus.is_elevated ? (
            <>
              <ShieldCheck className="w-3.5 h-3.5 text-[#91DB79]" />
              <span className="text-[#91DB79]">Elevated (Admin)</span>
            </>
          ) : (
            <>
              <ShieldAlert className="w-3.5 h-3.5 text-[#E9B65C]" />
              <span className="text-[#E9B65C]">Standard (Needs Admin for WFP)</span>
            </>
          )}
        </div>
      </div>
    </footer>
  );
};
