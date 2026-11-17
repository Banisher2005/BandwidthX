import React, { useState } from 'react';
import { 
  Zap, 
  ShieldAlert, 
  ShieldCheck, 
  CheckCircle2, 
  Search,
  Lock,
  Unlock,
  Radio,
  Server,
  Activity,
  ArrowDown,
  ArrowUp,
  AlertTriangle,
  RotateCcw
} from 'lucide-react';
import { ExclusiveModeState, ProcessNetworkInfo, ServiceStatus } from '../../types';
import { formatSpeed, formatBytes } from '../../utils/format';

interface ExclusiveModePageProps {
  exclusiveState: ExclusiveModeState;
  serviceStatus: ServiceStatus;
  processes: ProcessNetworkInfo[];
  onToggleExclusive: (targetPid?: number, exePath?: string, appName?: string, creationTime?: number) => void;
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
        onToggleExclusive(selectedProcess.pid, selectedProcess.exe_path, selectedProcess.name, selectedProcess.creation_time);
      }
    }
  };

  return (
    <div className="space-y-6 max-w-5xl mx-auto">
      {/* 1. Master Hero Banner */}
      <div className={`p-6 rounded-2xl border transition-all duration-300 ${
        exclusiveState.is_active 
          ? 'bg-status-danger/10 border-status-danger/40 shadow-2xl shadow-status-danger/10 backdrop-blur-xl' 
          : 'glass-card border-glass-border'
      }`}>
        <div className="flex flex-col md:flex-row items-center justify-between gap-6">
          <div className="space-y-2 text-center md:text-left">
            <div className="flex items-center justify-center md:justify-start space-x-2">
              <span className={`px-2.5 py-0.5 rounded-full text-xs font-mono font-bold uppercase tracking-wider flex items-center gap-1.5 ${
                exclusiveState.is_active 
                  ? 'bg-status-danger/20 text-status-danger border border-status-danger/40' 
                  : 'bg-white/[0.04] text-content-secondary border border-glass-border'
              }`}>
                <span className={`w-2 h-2 rounded-full ${exclusiveState.is_active ? 'bg-status-danger animate-pulse' : 'bg-content-muted'}`} />
                {exclusiveState.is_active ? 'EXCLUSIVE MODE ACTIVE' : 'NORMAL NETWORK MODE'}
              </span>
              {serviceStatus.is_elevated && (
                <span className="px-2 py-0.5 rounded-full text-[10px] font-mono bg-accent-primary/15 text-accent-primary border border-accent-primary/30">
                  WFP ALE BOUND
                </span>
              )}
            </div>

            <h2 className="text-2xl font-bold tracking-tight text-content-primary">
              Exclusive Bandwidth Mode
            </h2>
            <p className="text-sm text-content-secondary max-w-xl">
              Dedicate your entire network interface to a single prioritized application. All other applications are restricted from inbound and outbound traffic at the Windows Filtering Platform ALE kernel layer.
            </p>
          </div>

          {/* Large Master Toggle */}
          <div className="flex flex-col items-center space-y-2">
            <button
              onClick={handleToggle}
              disabled={!exclusiveState.is_active && !selectedProcess}
              className={`w-52 py-3.5 px-6 rounded-xl font-bold text-sm tracking-wide transition-all duration-200 flex items-center justify-center space-x-3 shadow-lg ${
                exclusiveState.is_active
                  ? 'bg-status-danger hover:bg-status-danger/90 text-background shadow-status-danger/30'
                  : 'bg-accent-primary hover:bg-accent-primary/90 text-background shadow-accent-primary/20'
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
            <span className="text-[11px] text-content-muted font-mono">
              {exclusiveState.is_active ? 'Restores all application traffic' : 'Enforces isolated ALE sublayer'}
            </span>
          </div>
        </div>
      </div>

      {/* 2. Target Process Card */}
      <div className="glass-card p-5 rounded-2xl border border-glass-border space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Radio className="w-4 h-4 text-accent-primary" />
            <h3 className="text-sm font-semibold text-content-primary">
              Target Application Identity
            </h3>
          </div>
          {exclusiveState.is_active && (
            <span className="text-xs font-mono text-accent-primary flex items-center gap-1.5">
              <CheckCircle2 className="w-3.5 h-3.5" />
              Verified & Permitted in WFP
            </span>
          )}
        </div>

        {selectedProcess ? (
          <div className="p-4 rounded-xl bg-background/60 border border-glass-border flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
            <div className="flex items-center space-x-4">
              <div className="w-12 h-12 rounded-xl bg-white/[0.04] border border-glass-border flex items-center justify-center text-lg font-mono font-bold text-accent-primary uppercase">
                {selectedProcess.name.slice(0, 2)}
              </div>
              <div className="space-y-1">
                <div className="flex items-center space-x-2">
                  <span className="text-base font-bold text-content-primary">
                    {selectedProcess.name}
                  </span>
                  <span className="text-xs font-mono px-2 py-0.5 rounded-full bg-white/[0.06] text-content-secondary border border-glass-border">
                    PID: {selectedProcess.pid}
                  </span>
                </div>
                <div className="text-xs font-mono text-content-muted break-all">
                  {selectedProcess.exe_path}
                </div>
              </div>
            </div>

            {/* Speeds & Sockets */}
            <div className="flex items-center space-x-6 text-right font-mono self-end md:self-auto">
              <div>
                <div className="text-sm font-bold text-accent-primary flex items-center gap-1 justify-end">
                  <ArrowDown className="w-3.5 h-3.5" />
                  {formatSpeed(selectedProcess.download_bytes_sec)}
                </div>
                <div className="text-xs text-accent-secondary flex items-center gap-1 justify-end">
                  <ArrowUp className="w-3 h-3" />
                  {formatSpeed(selectedProcess.upload_bytes_sec)}
                </div>
              </div>
              <div className="border-l border-glass-border pl-6 text-xs text-content-secondary">
                <div>{selectedProcess.connection_count} active sockets</div>
                <div>{formatBytes(selectedProcess.total_download_bytes + selectedProcess.total_upload_bytes)} total</div>
              </div>
            </div>
          </div>
        ) : (
          <div className="p-8 text-center text-sm text-content-muted bg-background/40 rounded-xl border border-glass-border">
            Please select an application below to enable Exclusive Mode.
          </div>
        )}
      </div>

      {/* 3. Status Counters */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Permitted Applications */}
        <div className="glass-card p-4 rounded-xl border border-glass-border">
          <div className="flex items-center justify-between text-content-secondary mb-1">
            <span className="text-xs font-medium">Permitted Applications</span>
            <ShieldCheck className="w-4 h-4 text-accent-primary" />
          </div>
          <div className="text-2xl font-bold font-mono text-content-primary">
            {exclusiveState.is_active ? 1 : processes.length}
          </div>
          <div className="text-[11px] text-content-muted mt-1">
            {exclusiveState.is_active ? 'Only selected target process' : 'All running applications'}
          </div>
        </div>

        {/* Restricted Applications */}
        <div className="glass-card p-4 rounded-xl border border-glass-border">
          <div className="flex items-center justify-between text-content-secondary mb-1">
            <span className="text-xs font-medium">Restricted Applications</span>
            <ShieldAlert className="w-4 h-4 text-status-danger" />
          </div>
          <div className="text-2xl font-bold font-mono text-content-primary">
            {exclusiveState.is_active ? Math.max(0, processes.length - 1) : 0}
          </div>
          <div className="text-[11px] text-content-muted mt-1">
            Restricted at ALE kernel layers
          </div>
        </div>

        {/* Core Windows Exemptions */}
        <div className="glass-card p-4 rounded-xl border border-glass-border">
          <div className="flex items-center justify-between text-content-secondary mb-1">
            <span className="text-xs font-medium">System Exemptions</span>
            <Server className="w-4 h-4 text-accent-secondary" />
          </div>
          <div className="text-2xl font-bold font-mono text-content-primary">
            {exclusiveState.allowed_exemptions.length}
          </div>
          <div className="text-[11px] text-accent-secondary mt-1 truncate" title={exclusiveState.allowed_exemptions.join(', ')}>
            DNS, DHCP, Loopback, System
          </div>
        </div>
      </div>

      {/* 4. Process Picker */}
      <div className="glass-card p-5 rounded-2xl border border-glass-border space-y-4">
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
          <div>
            <h3 className="text-sm font-semibold text-content-primary">
              Select Process to Prioritize
            </h3>
            <p className="text-xs text-content-secondary">
              Choose from currently running desktop applications
            </p>
          </div>

          <div className="relative w-full sm:w-64">
            <Search className="w-3.5 h-3.5 text-content-muted absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search running apps..."
              className="w-full pl-8 pr-3 py-1.5 rounded-lg bg-background/70 border border-glass-border text-xs text-content-primary placeholder-content-muted focus:outline-none focus:border-accent-primary/50"
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
                className={`p-3 rounded-xl border text-left transition-all duration-150 flex items-center justify-between ${
                  isSelected
                    ? 'glass-panel-elevated border-accent-primary/60 text-accent-primary shadow-sm'
                    : 'bg-background/40 border-glass-border hover:border-white/10 hover:bg-white/[0.02]'
                } disabled:cursor-not-allowed`}
              >
                <div className="min-w-0 pr-2">
                  <div className="font-semibold text-xs text-content-primary truncate">
                    {proc.name}
                  </div>
                  <div className="text-[10px] font-mono text-content-muted">
                    PID: {proc.pid}
                  </div>
                </div>

                <div className="text-right font-mono text-[11px] shrink-0">
                  <div className="text-accent-primary">
                    {formatSpeed(proc.download_bytes_sec)}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {/* 5. Safety & Verification Box */}
      <div className="glass-card p-4 rounded-xl border border-glass-border flex items-start space-x-3 text-xs text-content-secondary">
        <RotateCcw className="w-5 h-5 text-accent-secondary shrink-0 mt-0.5" />
        <div className="space-y-1">
          <div className="text-content-primary font-semibold">
            Dynamic Kernel Session Safety Guarantee
          </div>
          <p className="leading-relaxed">
            BandwidthX uses dynamic WFP sessions (<code className="font-mono text-accent-primary">FWPM_SESSION_FLAG_DYNAMIC</code>). All ALE sublayer authorization filters are owned exclusively by BandwidthX. If this application is closed, terminated, or if your PC restarts, Windows immediately and automatically purges the filters, ensuring you are never locked out of your network.
          </p>
        </div>
      </div>
    </div>
  );
};