import React from 'react';
import { Shield, Zap, AlertTriangle, RefreshCw } from 'lucide-react';
import { ExclusiveModeState, ServiceStatus } from '../../types';

interface HeaderProps {
  exclusiveState: ExclusiveModeState;
  serviceStatus: ServiceStatus;
  onEmergencyRestore: () => void;
  onRefresh: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  exclusiveState,
  serviceStatus,
  onEmergencyRestore,
  onRefresh,
}) => {
  return (
    <header className="h-16 border-b border-[#192223] bg-[#0B0F10]/90 backdrop-blur-md px-6 flex items-center justify-between z-20 select-none">
      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-3">
          <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-[#91DB79] to-[#57C7B5] flex items-center justify-center shadow-lg shadow-[#91DB79]/10">
            <Zap className="w-5 h-5 text-[#0B0F10] font-black" />
          </div>
          <div>
            <h1 className="text-lg font-bold tracking-tight text-[#EDF5F0] flex items-center gap-2">
              BandwidthX
              <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-[#192223] text-[#57C7B5] border border-[#57C7B5]/30">
                PRO v1.0
              </span>
            </h1>
            <p className="text-xs text-[#91A39A] font-medium tracking-wide">
              Your Network. Your Rules.
            </p>
          </div>
        </div>
      </div>

      <div className="flex items-center space-x-3">
        {/* Exclusive mode active indicator */}
        {exclusiveState.is_active && (
          <div className="flex items-center space-x-2 px-3 py-1.5 rounded-full bg-[#EF6B70]/10 border border-[#EF6B70]/40 text-[#EF6B70] text-xs font-semibold animate-pulse">
            <span className="w-2 h-2 rounded-full bg-[#EF6B70]"></span>
            <span>EXCLUSIVE MODE: {exclusiveState.target_app_name}</span>
          </div>
        )}

        {/* Elevated privilege badge */}
        {serviceStatus.is_elevated && (
          <div className="flex items-center space-x-1.5 px-2.5 py-1 rounded bg-[#192223] border border-[#91DB79]/30 text-[#91DB79] text-xs">
            <Shield className="w-3.5 h-3.5" />
            <span className="font-mono text-[11px]">ADMIN WFP</span>
          </div>
        )}

        {/* Manual refresh button */}
        <button
          onClick={onRefresh}
          className="p-2 rounded-lg bg-[#12191A] hover:bg-[#192223] text-[#91A39A] hover:text-[#EDF5F0] border border-[#192223] transition-colors"
          title="Refresh Network Telemetry"
        >
          <RefreshCw className="w-4 h-4" />
        </button>

        {/* Emergency Restore Button */}
        <button
          onClick={onEmergencyRestore}
          className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-[#EF6B70]/15 hover:bg-[#EF6B70]/25 border border-[#EF6B70]/40 text-[#EF6B70] text-xs font-medium transition-all duration-150"
          title="Emergency rollback: Instantly removes all BandwidthX WFP filters and restores normal networking"
        >
          <AlertTriangle className="w-3.5 h-3.5" />
          <span>Emergency Restore</span>
        </button>
      </div>
    </header>
  );
};
