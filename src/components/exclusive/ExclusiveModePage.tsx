import React, { useState } from 'react';
import { 
  Zap, 
  ShieldAlert, 
  ShieldCheck, 
  CheckCircle2, 
  AlertTriangle, 
  Info, 
  Search,
  Lock,
  Unlock,
  Radio,
  Server
} from 'lucide-react';
import { ExclusiveModeState, ProcessNetworkInfo, ServiceStatus } from '../../types';
import { formatSpeed, formatBytes } from '../../utils/format';

interface ExclusiveModePageProps {
  exclusiveState: ExclusiveModeState;
  serviceStatus: ServiceStatus;
  processes: ProcessNetworkInfo[];
  onToggleExclusive: (targetPid?: number, exePath?: string, appName?: string) => void;
  onEmergencyRestore: () => void;
}

export const ExclusiveModePage: React.FC<ExclusiveModePageProps> = ({
  exclusiveState,
  serviceStatus,
  processes,
  onToggleExclusive,
  onEmergencyRestore,
}) => {
  const [selectedProcess, setSelectedProcess] = useState<ProcessNetworkInfo | null>(() => {
    if (exclusiveState.is_active && exclusiveState.target_exe_path) {
      return processes.find((p) => p.exe_path === exclusiveState.target_exe_path) || null;
    }
    return processes[0] || null;
  });

  const [searchQuery, setSearchQuery] = useState('');

  // Filter processes for selection
  const selectableProcesses = processes.filter((proc) => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase();
    return proc.name.toLowerCase().includes(q) || proc.exe_path.toLowerCase().includes(q);
  });

  const handleToggle = () => {
    if (exclusiveState.is_active) {
      onToggleExclusive();
    } else {
      if (selectedProcess) {
        onToggleExclusive(selectedProcess.pid, selectedProcess.exe_path, selectedProcess.name);
      }
    }
  };

  return (
    <div className="space-y-6 max-w-5xl mx-auto">
      {/* 1. Hero Header Banner */}
      <div className={`p-6 rounded-2xl border transition-all duration-300 ${
        exclusiveState.is_active 
          ? 'bg-gradient-to-r from-[#192223] via-[#12191A] to-[#0B0F10] border-[#EF6B70]/50 shadow-2xl shadow-[#EF6B70]/10' 
          : 'bg-[#12191A] border-[#192223]'
      }`}>
        <div className="flex flex-col md:flex-row items-center justify-between gap-6">
          <div className="space-y-2 text-center md:text-left">
            <div className="flex items-center justify-center md:justify-start space-x-2">
              <span className={`px-2.5 py-0.5 rounded text-xs font-mono font-bold uppercase tracking-wider ${
                exclusiveState.is_active 
                  ? 'bg-[#EF6B70]/20 text-[#EF6B70] border border-[#EF6B70]/40' 
                  : 'bg-[#192223] text-[#91A39A] border border-[#212D2E]'
              }`}>
                {exclusiveState.is_active ? '● EXCLUSIVE MODE ACTIVE' : '○ NORMAL NETWORK MODE'}
              </span>
              {serviceStatus.is_elevated && (
                <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-[#91DB79]/15 text-[#91DB79] border border-[#91DB79]/30">
                  WFP ALE READY
                </span>
              )}
            </div>

            <h2 className="text-2xl font-bold tracking-tight text-[#EDF5F0]">
              Exclusive Bandwidth Mode
            </h2>
            <p className="text-sm text-[#91A39A] max-w-xl">
              Dedicate your entire network interface to a single prioritized application. All other applications are restricted from inbound and outbound traffic at the Windows Filtering Platform ALE kernel layer.
            </p>
          </div>

          {/* Large Master Toggle */}
          <div className="flex flex-col items-center space-y-2">
            <button
              onClick={handleToggle}
              disabled={!exclusiveState.is_active && !selectedProcess}
              className={`w-48 py-3.5 px-6 rounded-xl font-bold text-sm tracking-wide transition-all duration-200 flex items-center justify-center space-x-3 shadow-lg ${
                exclusiveState.is_active
                  ? 'bg-[#EF6B70] hover:bg-[#EF6B70]/90 text-[#0B0F10] shadow-[#EF6B70]/30'
                  : 'bg-[#91DB79] hover:bg-[#91DB79]/90 text-[#0B0F10] shadow-[#91DB79]/20'
              } disabled:opacity-40 disabled:cursor-not-allowed`}
            >
              {exclusiveState.is_active ? (
                <>
                  <Unlock className="w-5 h-5" />
                  <span>DISABLE EXCLUSIVE</span>
                </>
              ) : (
                <>
                  <Lock className="w-5 h-5" />
                  <span>ENABLE EXCLUSIVE</span>
                </>
              )}
            </button>
            <span className="text-[11px] text-[#91A39A] font-mono">
              {exclusiveState.is_active ? 'Click to restore all traffic' : 'Activates isolated sublayer'}
            </span>
          </div>
        </div>
      </div>

      {/* 2. Selected Application Details Card */}
      <div className="p-5 rounded-xl bg-[#12191A] border border-[#192223] space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Radio className="w-4 h-4 text-[#91DB79]" />
            <h3 className="text-sm font-semibold text-[#EDF5F0]">
              Target Application Identity
            </h3>
          </div>
          {exclusiveState.is_active && (
            <span className="text-xs font-mono text-[#91DB79] flex items-center gap-1.5">
              <CheckCircle2 className="w-3.5 h-3.5" />
              Verified & Permitted in WFP
            </span>
          )}
        </div>

        {selectedProcess ? (
          <div className="p-4 rounded-lg bg-[#0B0F10] border border-[#192223] flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
            <div className="flex items-center space-x-4">
              <div className="w-12 h-12 rounded-xl bg-[#192223] border border-[#212D2E] flex items-center justify-center text-lg font-mono font-bold text-[#91DB79] uppercase">
                {selectedProcess.name.slice(0, 2)}
              </div>
              <div className="space-y-1">
                <div className="flex items-center space-x-2">
                  <span className="text-base font-bold text-[#EDF5F0]">
                    {selectedProcess.name}
                  </span>
                  <span className="text-xs font-mono px-2 py-0.5 rounded bg-[#192223] text-[#91A39A]">
                    PID: {selectedProcess.pid}
                  </span>
                </div>
                <div className="text-xs font-mono text-[#91A39A] break-all">
                  {selectedProcess.exe_path}
                </div>
              </div>
            </div>

            {/* Speeds & Sockets */}
            <div className="flex items-center space-x-6 text-right font-mono self-end md:self-auto">
              <div>
                <div className="text-sm font-bold text-[#91DB79]">
                  ↓ {formatSpeed(selectedProcess.download_bytes_sec)}
                </div>
                <div className="text-xs text-[#57C7B5]">
                  ↑ {formatSpeed(selectedProcess.upload_bytes_sec)}
                </div>
              </div>
              <div className="border-l border-[#192223] pl-6 text-xs text-[#91A39A]">
                <div>{selectedProcess.connection_count} active sockets</div>
                <div>{formatBytes(selectedProcess.total_download_bytes + selectedProcess.total_upload_bytes)} total</div>
              </div>
            </div>
          </div>
        ) : (
          <div className="p-8 text-center text-sm text-[#91A39A] bg-[#0B0F10] rounded-lg border border-[#192223]">
            Please select an application below to enable Exclusive Mode.
          </div>
        )}
      </div>

      {/* 3. Telemetry Counters & System Safeguards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Allowed Applications */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223]">
          <div className="flex items-center justify-between text-[#91A39A] mb-1">
            <span className="text-xs font-medium">Permitted Applications</span>
            <ShieldCheck className="w-4 h-4 text-[#91DB79]" />
          </div>
          <div className="text-2xl font-bold font-mono text-[#EDF5F0]">
            {exclusiveState.is_active ? 1 : processes.length}
          </div>
          <div className="text-[11px] text-[#91A39A] mt-1">
            {exclusiveState.is_active ? 'Only selected target process' : 'All running applications'}
          </div>
        </div>

        {/* Restricted Applications */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223]">
          <div className="flex items-center justify-between text-[#91A39A] mb-1">
            <span className="text-xs font-medium">Restricted Applications</span>
            <ShieldAlert className="w-4 h-4 text-[#EF6B70]" />
          </div>
          <div className="text-2xl font-bold font-mono text-[#EDF5F0]">
            {exclusiveState.is_active ? Math.max(0, processes.length - 1) : 0}
          </div>
          <div className="text-[11px] text-[#91A39A] mt-1">
            Denied network at ALE layers
          </div>
        </div>

        {/* System Core Exemptions */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223]">
          <div className="flex items-center justify-between text-[#91A39A] mb-1">
            <span className="text-xs font-medium">Core Windows Exemptions</span>
            <Server className="w-4 h-4 text-[#57C7B5]" />
          </div>
          <div className="text-2xl font-bold font-mono text-[#EDF5F0]">
            {exclusiveState.allowed_exemptions.length}
          </div>
          <div className="text-[11px] text-[#57C7B5] mt-1 truncate" title={exclusiveState.allowed_exemptions.join(', ')}>
            DNS, DHCP, Loopback, System
          </div>
        </div>
      </div>

      {/* 4. Process Picker */}
      <div className="p-5 rounded-xl bg-[#12191A] border border-[#192223] space-y-4">
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
          <div>
            <h3 className="text-sm font-semibold text-[#EDF5F0]">
              Select Process to Prioritize
            </h3>
            <p className="text-xs text-[#91A39A]">
              Choose from currently running desktop applications
            </p>
          </div>

          <div className="relative w-full sm:w-64">
            <Search className="w-3.5 h-3.5 text-[#91A39A] absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search running apps..."
              className="w-full pl-8 pr-3 py-1.5 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] placeholder-[#62736A] focus:outline-none focus:border-[#91DB79]/50"
            />
          </div>
        </div>

        {/* Process Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-2.5 max-h-60 overflow-y-auto pr-1">
          {selectableProcesses.map((proc) => {
            const isSelected = selectedProcess?.pid === proc.pid;
            return (
              <button
                key={`${proc.pid}-${proc.name}`}
                onClick={() => setSelectedProcess(proc)}
                disabled={exclusiveState.is_active}
                className={`p-3 rounded-lg border text-left transition-all duration-150 flex items-center justify-between ${
                  isSelected
                    ? 'bg-[#192223] border-[#91DB79] shadow-sm'
                    : 'bg-[#0B0F10] border-[#192223] hover:border-[#212D2E]'
                } disabled:cursor-not-allowed`}
              >
                <div className="min-w-0 pr-2">
                  <div className="font-semibold text-xs text-[#EDF5F0] truncate">
                    {proc.name}
                  </div>
                  <div className="text-[10px] font-mono text-[#91A39A]">
                    PID: {proc.pid}
                  </div>
                </div>

                <div className="text-right font-mono text-[11px] shrink-0">
                  <div className="text-[#91DB79]">
                    {formatSpeed(proc.download_bytes_sec)}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {/* 5. Safety & Verification Box */}
      <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex items-start space-x-3 text-xs text-[#91A39A]">
        <Info className="w-5 h-5 text-[#57C7B5] shrink-0 mt-0.5" />
        <div className="space-y-1">
          <div className="text-[#EDF5F0] font-semibold">
            Dynamic Kernel Session Safety Guarantee
          </div>
          <p className="leading-relaxed">
            BandwidthX uses dynamic WFP sessions (`FWPM_SESSION_FLAG_DYNAMIC`). All ALE sublayer authorization filters are owned exclusively by BandwidthX. If this application is closed, terminated, or if your PC restarts, Windows immediately and automatically purges the filters, ensuring you are never locked out of your network.
          </p>
        </div>
      </div>
    </div>
  );
};
