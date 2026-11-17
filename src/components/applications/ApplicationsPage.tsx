import React, { useState, useMemo } from 'react';
import { 
  Search, 
  ArrowUpDown, 
  ShieldCheck, 
  ShieldX, 
  Sliders, 
  Globe, 
  X, 
  Info, 
  Skull, 
  AlertTriangle, 
  RotateCw, 
  CheckCircle2, 
  Layers,
  ArrowDown,
  ArrowUp,
  Radio
} from 'lucide-react';
import { ProcessNetworkInfo, NetworkRule } from '../../types';
import { formatBytes, formatSpeed, truncateMiddle } from '../../utils/format';
import { api } from '../../services/api';

interface ApplicationsPageProps {
  processes: ProcessNetworkInfo[];
  rules: NetworkRule[];
  onToggleBlock: (process: ProcessNetworkInfo) => void;
  onSelectExclusive: (process: ProcessNetworkInfo) => void;
  onRefresh: () => void;
  onNotification: (msg: string) => void;
}

type SortField = 'speed' | 'download' | 'upload' | 'total' | 'name' | 'pid' | 'connections';

export const ApplicationsPage: React.FC<ApplicationsPageProps> = ({
  processes,
  rules,
  onToggleBlock,
  onSelectExclusive,
  onRefresh,
  onNotification,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<'all' | 'active' | 'blocked' | 'allowed' | 'recent'>('all');
  const [sortField, setSortField] = useState<SortField>('speed');
  const [sortAsc, setSortAsc] = useState(false);
  const [selectedProcess, setSelectedProcess] = useState<ProcessNetworkInfo | null>(null);
  
  // Terminate confirmation state
  const [terminateTarget, setTerminateTarget] = useState<ProcessNetworkInfo | null>(null);
  const [isTerminating, setIsTerminating] = useState(false);

  // Filter & sort logic
  const filteredProcesses = useMemo(() => {
    return processes.filter((proc) => {
      // Search query
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesName = proc.name.toLowerCase().includes(query);
        const matchesPid = proc.pid.toString().includes(query);
        const matchesPath = proc.exe_path.toLowerCase().includes(query);
        if (!matchesName && !matchesPid && !matchesPath) return false;
      }

      // Filter tabs
      if (filterType === 'active') {
        return proc.download_bytes_sec > 0 || proc.upload_bytes_sec > 0;
      }
      if (filterType === 'blocked') {
        return proc.is_blocked;
      }
      if (filterType === 'allowed') {
        return !proc.is_blocked;
      }
      if (filterType === 'recent') {
        return proc.connection_count > 0 || proc.total_download_bytes > 0;
      }
      return true;
    }).sort((a, b) => {
      let comparison = 0;
      if (sortField === 'speed') {
        const speedA = a.download_bytes_sec + a.upload_bytes_sec;
        const speedB = b.download_bytes_sec + b.upload_bytes_sec;
        comparison = speedB - speedA;
      } else if (sortField === 'download') {
        comparison = b.download_bytes_sec - a.download_bytes_sec;
      } else if (sortField === 'upload') {
        comparison = b.upload_bytes_sec - a.upload_bytes_sec;
      } else if (sortField === 'total') {
        const totalA = a.total_download_bytes + a.total_upload_bytes;
        const totalB = b.total_download_bytes + b.total_upload_bytes;
        comparison = totalB - totalA;
      } else if (sortField === 'connections') {
        comparison = b.connection_count - a.connection_count;
      } else if (sortField === 'name') {
        comparison = a.name.localeCompare(b.name);
      } else if (sortField === 'pid') {
        comparison = a.pid - b.pid;
      }
      return sortAsc ? -comparison : comparison;
    });
  }, [processes, searchQuery, filterType, sortField, sortAsc]);

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortAsc(!sortAsc);
    } else {
      setSortField(field);
      setSortAsc(false);
    }
  };

  const executeTermination = async () => {
    if (!terminateTarget) return;

    try {
      setIsTerminating(true);
      await api.terminateProcess(terminateTarget.pid, terminateTarget.creation_time);
      onNotification(`Successfully terminated ${terminateTarget.name} (PID ${terminateTarget.pid})`);
      setTerminateTarget(null);
      if (selectedProcess?.pid === terminateTarget.pid) {
        setSelectedProcess(null);
      }
      onRefresh();
    } catch (err: any) {
      onNotification(`Termination failed: ${err?.message || err}`);
    } finally {
      setIsTerminating(false);
    }
  };

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-10">
      {/* Header and Summary */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center space-x-2.5">
            <div className="p-2 rounded-xl bg-accent-primary/15 border border-accent-primary/30 text-accent-primary">
              <Layers className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-xl font-bold text-content-primary tracking-tight">Active Applications</h2>
              <p className="text-xs text-content-secondary mt-0.5">
                Real-time per-process socket attribution via Windows TCP EStats.
              </p>
            </div>
          </div>
        </div>

        {/* Telemetry Source Tooltip / Notice */}
        <div className="flex items-center space-x-2 text-xs">
          <div 
            className="px-3 py-1.5 rounded-xl glass-card text-content-secondary flex items-center space-x-1.5 cursor-help"
            title="TCP bytes are measured directly from live socket counters (TcpConnectionEstatsData). UDP sockets are tracked by endpoint presence; per-process UDP byte counts require kernel callout driver."
          >
            <Info className="w-3.5 h-3.5 text-accent-secondary" />
            <span>TCP EStats Verified</span>
          </div>

          <button 
            onClick={onRefresh}
            className="p-1.5 rounded-xl glass-card text-content-secondary hover:text-content-primary glass-card-hover"
            title="Refresh Process List"
          >
            <RotateCw className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Search Bar & Filters */}
      <div className="flex flex-col sm:flex-row gap-3">
        <div className="relative flex-1">
          <Search className="w-4 h-4 text-content-muted absolute left-3.5 top-1/2 -translate-y-1/2" />
          <input 
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search process by name, PID, or executable path..."
            className="w-full pl-10 pr-4 py-2.5 rounded-2xl glass-card bg-white/[0.03] text-sm text-content-primary placeholder:text-content-muted focus:outline-none focus:border-accent-primary/50 transition-colors"
          />
        </div>

        <div className="flex items-center space-x-1.5 p-1 rounded-2xl glass-card shrink-0 text-xs overflow-x-auto">
          {(['all', 'active', 'blocked', 'allowed', 'recent'] as const).map((t) => (
            <button
              key={t}
              onClick={() => setFilterType(t)}
              className={`px-3 py-1.5 rounded-xl font-medium capitalize transition-all whitespace-nowrap ${
                filterType === t 
                  ? 'bg-accent-primary text-background font-bold shadow-md' 
                  : 'text-content-secondary hover:text-content-primary'
              }`}
            >
              {t === 'active' ? 'Using Network' : t}
            </button>
          ))}
        </div>
      </div>

      {/* Applications Table */}
      <div className="rounded-3xl glass-card overflow-hidden">
        {filteredProcesses.length === 0 ? (
          <div className="py-16 text-center text-content-muted text-sm">
            No applications match your filter criteria.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs border-collapse">
              <thead>
                <tr className="border-b border-white/[0.06] text-content-muted uppercase tracking-wider font-mono text-[10px]">
                  <th 
                    onClick={() => handleSort('name')}
                    className="py-3.5 px-4 font-semibold cursor-pointer hover:text-content-primary select-none"
                  >
                    <div className="flex items-center space-x-1">
                      <span>Application</span>
                      <ArrowUpDown className="w-3 h-3 opacity-60" />
                    </div>
                  </th>
                  <th 
                    onClick={() => handleSort('pid')}
                    className="py-3.5 px-3 font-semibold cursor-pointer hover:text-content-primary select-none"
                  >
                    PID
                  </th>
                  <th 
                    onClick={() => handleSort('download')}
                    className="py-3.5 px-3 font-semibold cursor-pointer hover:text-content-primary select-none"
                  >
                    <div className="flex items-center space-x-1">
                      <span>Download</span>
                      <ArrowDown className="w-3 h-3 text-accent-primary" />
                    </div>
                  </th>
                  <th 
                    onClick={() => handleSort('upload')}
                    className="py-3.5 px-3 font-semibold cursor-pointer hover:text-content-primary select-none"
                  >
                    <div className="flex items-center space-x-1">
                      <span>Upload</span>
                      <ArrowUp className="w-3 h-3 text-accent-secondary" />
                    </div>
                  </th>
                  <th 
                    onClick={() => handleSort('total')}
                    className="py-3.5 px-3 font-semibold cursor-pointer hover:text-content-primary select-none"
                  >
                    Total
                  </th>
                  <th className="py-3.5 px-3 font-semibold">Status</th>
                  <th className="py-3.5 px-4 font-semibold text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/[0.04]">
                {filteredProcesses.map((proc) => {
                  const hasActiveSpeed = proc.download_bytes_sec > 0 || proc.upload_bytes_sec > 0;
                  const isBlocked = proc.is_blocked;
                  const isExclusive = proc.is_exclusive_target;

                  return (
                    <tr 
                      key={`${proc.pid}-${proc.name}`}
                      className="hover:bg-white/[0.025] transition-colors group cursor-pointer"
                      onClick={() => setSelectedProcess(proc)}
                    >
                      {/* Name & Path */}
                      <td className="py-3 px-4 max-w-xs">
                        <div className="flex items-center space-x-2.5">
                          <div className="w-7 h-7 rounded-lg bg-white/[0.05] border border-white/[0.08] flex items-center justify-center shrink-0">
                            <Globe className="w-3.5 h-3.5 text-accent-secondary" />
                          </div>
                          <div className="min-w-0">
                            <div className="font-semibold text-content-primary truncate flex items-center space-x-1.5">
                              <span>{proc.name}</span>
                              {isExclusive && (
                                <span className="text-[9px] px-1.5 py-0.2 rounded bg-accent-primary text-background font-bold">
                                  TARGET
                                </span>
                              )}
                            </div>
                            <div className="text-[10px] font-mono text-content-muted truncate" title={proc.exe_path}>
                              {truncateMiddle(proc.exe_path, 36)}
                            </div>
                          </div>
                        </div>
                      </td>

                      {/* PID */}
                      <td className="py-3 px-3 font-mono text-content-secondary">
                        <span className="px-1.5 py-0.5 rounded bg-white/[0.04] border border-white/[0.08]">
                          {proc.pid}
                        </span>
                      </td>

                      {/* Download Throughput */}
                      <td className="py-3 px-3 font-mono font-medium">
                        {proc.download_bytes_sec > 0 ? (
                          <span className="text-accent-primary font-bold">
                            {formatSpeed(proc.download_bytes_sec)}
                          </span>
                        ) : (
                          <span className="text-content-muted">0 B/s</span>
                        )}
                      </td>

                      {/* Upload Throughput */}
                      <td className="py-3 px-3 font-mono font-medium">
                        {proc.upload_bytes_sec > 0 ? (
                          <span className="text-accent-secondary font-bold">
                            {formatSpeed(proc.upload_bytes_sec)}
                          </span>
                        ) : (
                          <span className="text-content-muted">0 B/s</span>
                        )}
                      </td>

                      {/* Total Measured Traffic */}
                      <td className="py-3 px-3 font-mono text-content-secondary">
                        {formatBytes(proc.total_download_bytes + proc.total_upload_bytes)}
                      </td>

                      {/* Status */}
                      <td className="py-3 px-3">
                        {isBlocked ? (
                          <span className="inline-flex items-center space-x-1 text-status-danger font-semibold text-[11px]">
                            <ShieldX className="w-3.5 h-3.5" />
                            <span>Blocked</span>
                          </span>
                        ) : hasActiveSpeed ? (
                          <span className="inline-flex items-center space-x-1 text-accent-primary font-semibold text-[11px]">
                            <span className="w-1.5 h-1.5 rounded-full bg-accent-primary animate-pulse"></span>
                            <span>Active</span>
                          </span>
                        ) : (
                          <span className="text-content-muted text-[11px]">
                            Idle ({proc.connection_count} sockets)
                          </span>
                        )}
                      </td>

                      {/* Separate Actions: Block/Allow, Terminate */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <div className="flex items-center justify-end space-x-1.5">
                          {/* Block / Allow Toggle */}
                          <button
                            onClick={() => onToggleBlock(proc)}
                            className={`px-2.5 py-1 rounded-xl text-[11px] font-medium transition-all ${
                              isBlocked
                                ? 'bg-status-danger/15 hover:bg-status-danger/25 text-status-danger border border-status-danger/30'
                                : 'bg-white/[0.04] hover:bg-white/[0.08] text-content-secondary hover:text-content-primary border border-white/[0.08]'
                            }`}
                            title={isBlocked ? "Allow Network Access" : "Block Network Access via WFP"}
                          >
                            {isBlocked ? 'Allow' : 'Block'}
                          </button>

                          {/* Terminate Process */}
                          <button
                            onClick={() => setTerminateTarget(proc)}
                            className="p-1.5 rounded-xl bg-status-danger/10 hover:bg-status-danger/25 text-status-danger border border-status-danger/20 transition-all"
                            title={`Terminate ${proc.name} (PID ${proc.pid})`}
                          >
                            <Skull className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* TERMINATE CONFIRMATION MODAL                                      */}
      {/* ----------------------------------------------------------------- */}
      {terminateTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-background/80 backdrop-blur-md">
          <div className="max-w-md w-full rounded-3xl glass-panel-elevated p-6 border border-status-danger/30 shadow-2xl space-y-4 animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-center space-x-3 text-status-danger">
              <div className="p-2.5 rounded-2xl bg-status-danger/15 border border-status-danger/30">
                <AlertTriangle className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-base font-bold text-content-primary">Confirm Termination</h3>
                <p className="text-xs text-content-secondary">This action cannot be undone.</p>
              </div>
            </div>

            <p className="text-xs text-content-secondary leading-relaxed">
              Are you sure you want to terminate <strong className="text-content-primary font-mono">{terminateTarget.name}</strong> (PID {terminateTarget.pid})?
              Only this specific process instance will be terminated.
            </p>

            <div className="p-3 rounded-2xl bg-white/[0.025] border border-white/[0.06] font-mono text-[11px] text-content-muted break-all">
              {terminateTarget.exe_path}
            </div>

            <div className="flex justify-end space-x-3 pt-2">
              <button
                onClick={() => setTerminateTarget(null)}
                disabled={isTerminating}
                className="px-4 py-2 rounded-xl glass-card text-content-secondary text-xs hover:text-content-primary"
              >
                Cancel
              </button>
              <button
                onClick={executeTermination}
                disabled={isTerminating}
                className="flex items-center space-x-1.5 px-4 py-2 rounded-xl bg-status-danger hover:bg-status-danger/90 text-background font-bold text-xs shadow-lg transition-all"
              >
                {isTerminating ? (
                  <>
                    <RotateCw className="w-3.5 h-3.5 animate-spin" />
                    <span>Terminating...</span>
                  </>
                ) : (
                  <>
                    <Skull className="w-3.5 h-3.5" />
                    <span>Terminate Process</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* APPLICATION DETAILS DRAWER / MODAL                                */}
      {/* ----------------------------------------------------------------- */}
      {selectedProcess && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-background/80 backdrop-blur-md">
          <div className="max-w-lg w-full rounded-3xl glass-panel-elevated p-6 border border-white/10 shadow-2xl space-y-5 animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-start justify-between">
              <div>
                <div className="flex items-center space-x-2">
                  <Globe className="w-4 h-4 text-accent-secondary" />
                  <h3 className="text-base font-bold text-content-primary tracking-tight">
                    {selectedProcess.name}
                  </h3>
                </div>
                <div className="text-xs font-mono text-content-muted mt-0.5">
                  PID: {selectedProcess.pid} • Sockets: {selectedProcess.connection_count}
                </div>
              </div>
              <button 
                onClick={() => setSelectedProcess(null)}
                className="p-1 rounded-xl text-content-muted hover:text-content-primary glass-card"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-3 text-xs">
              <div className="p-3 rounded-2xl bg-white/[0.025] border border-white/[0.06] space-y-2">
                <div className="flex justify-between">
                  <span className="text-content-secondary">Current Download</span>
                  <span className="font-mono font-bold text-accent-primary">{formatSpeed(selectedProcess.download_bytes_sec)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">Current Upload</span>
                  <span className="font-mono font-bold text-accent-secondary">{formatSpeed(selectedProcess.upload_bytes_sec)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">Total Inbound</span>
                  <span className="font-mono text-content-primary">{formatBytes(selectedProcess.total_download_bytes)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">Total Outbound</span>
                  <span className="font-mono text-content-primary">{formatBytes(selectedProcess.total_upload_bytes)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">TCP Connections</span>
                  <span className="font-mono text-content-primary">{selectedProcess.active_tcp_connections}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">UDP Endpoints</span>
                  <span className="font-mono text-content-primary">{selectedProcess.active_udp_endpoints}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">WFP Policy</span>
                  <span className={`font-semibold ${selectedProcess.is_blocked ? 'text-status-danger' : 'text-accent-primary'}`}>
                    {selectedProcess.is_blocked ? 'BLOCKED (Kernel ALE)' : 'ALLOWED'}
                  </span>
                </div>
              </div>

              <div className="p-3 rounded-2xl bg-white/[0.025] border border-white/[0.06] space-y-1">
                <span className="text-content-secondary block text-[11px]">Executable Path</span>
                <span className="font-mono text-[11px] text-content-primary break-all select-all block">
                  {selectedProcess.exe_path}
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between pt-2">
              <button
                onClick={() => onSelectExclusive(selectedProcess)}
                className="flex items-center space-x-1.5 px-4 py-2 rounded-xl bg-accent-primary/10 hover:bg-accent-primary/20 text-accent-primary border border-accent-primary/30 text-xs font-semibold transition-all"
              >
                <span>Target in Exclusive Mode</span>
              </button>

              <button
                onClick={() => setSelectedProcess(null)}
                className="px-5 py-2 rounded-xl glass-card text-content-primary font-medium text-xs hover:bg-white/[0.08]"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
