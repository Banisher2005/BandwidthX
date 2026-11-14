import React, { useState, useMemo } from 'react';
import { 
  Search, 
  Filter, 
  ArrowUpDown, 
  ShieldCheck, 
  ShieldX, 
  Zap, 
  Sliders, 
  ChevronDown, 
  ChevronUp,
  Globe,
  Gauge,
  X,
  Info
} from 'lucide-react';
import { ProcessNetworkInfo, NetworkRule } from '../../types';
import { formatBytes, formatSpeed, truncateMiddle } from '../../utils/format';

interface ApplicationsPageProps {
  processes: ProcessNetworkInfo[];
  rules: NetworkRule[];
  onToggleBlock: (process: ProcessNetworkInfo) => void;
  onSelectExclusive: (process: ProcessNetworkInfo) => void;
  onSetLimit: (process: ProcessNetworkInfo) => void;
  onRefresh: () => void;
}

type SortField = 'speed' | 'download' | 'upload' | 'total' | 'name' | 'pid' | 'connections';

export const ApplicationsPage: React.FC<ApplicationsPageProps> = ({
  processes,
  rules,
  onToggleBlock,
  onSelectExclusive,
  onSetLimit,
  onRefresh,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<'all' | 'active' | 'blocked' | 'rules'>('all');
  const [sortField, setSortField] = useState<SortField>('speed');
  const [sortAsc, setSortAsc] = useState(false);
  const [selectedProcess, setSelectedProcess] = useState<ProcessNetworkInfo | null>(null);
  const [limitModalProc, setLimitModalProc] = useState<ProcessNetworkInfo | null>(null);
  const [limitValueKbps, setLimitValueKbps] = useState('2048');
  const [limitType, setLimitType] = useState<'download_limit' | 'upload_limit' | 'bidirectional_limit'>('download_limit');

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
        return proc.download_bytes_sec > 0 || proc.upload_bytes_sec > 0 || proc.connection_count > 0;
      }
      if (filterType === 'blocked') {
        return proc.is_blocked;
      }
      if (filterType === 'rules') {
        return proc.has_rules;
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

  return (
    <div className="space-y-4">
      {/* Search, Filter & Refresh Bar */}
      <div className="flex flex-col md:flex-row items-center justify-between gap-3 p-4 rounded-xl bg-[#12191A] border border-[#192223]">
        {/* Search */}
        <div className="relative w-full md:w-80">
          <Search className="w-4 h-4 text-[#91A39A] absolute left-3.5 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search processes, PID, or path..."
            className="w-full pl-10 pr-4 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-sm text-[#EDF5F0] placeholder-[#62736A] focus:outline-none focus:border-[#91DB79]/50 transition-colors"
          />
        </div>

        {/* Filter Tabs */}
        <div className="flex items-center space-x-1.5 self-start md:self-auto overflow-x-auto w-full md:w-auto">
          {[
            { id: 'all', label: `All (${processes.length})` },
            { id: 'active', label: 'Active Now' },
            { id: 'blocked', label: 'Blocked' },
            { id: 'rules', label: 'With Rules' },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setFilterType(tab.id as any)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                filterType === tab.id
                  ? 'bg-[#192223] text-[#91DB79] border border-[#91DB79]/30'
                  : 'text-[#91A39A] hover:bg-[#192223]/50 hover:text-[#EDF5F0]'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Applications Table */}
      <div className="rounded-xl bg-[#12191A] border border-[#192223] overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-[#0B0F10]/60 border-b border-[#192223] text-[#91A39A] font-mono select-none">
              <tr>
                <th className="py-3 px-4 cursor-pointer hover:text-[#EDF5F0]" onClick={() => handleSort('name')}>
                  <div className="flex items-center space-x-1">
                    <span>Application / Executable</span>
                    {sortField === 'name' && (sortAsc ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)}
                  </div>
                </th>
                <th className="py-3 px-3 cursor-pointer hover:text-[#EDF5F0]" onClick={() => handleSort('pid')}>
                  <div className="flex items-center space-x-1">
                    <span>PID</span>
                    {sortField === 'pid' && (sortAsc ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)}
                  </div>
                </th>
                <th className="py-3 px-3 cursor-pointer hover:text-[#EDF5F0]" onClick={() => handleSort('download')}>
                  <div className="flex items-center space-x-1">
                    <span>Download</span>
                    {sortField === 'download' && (sortAsc ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)}
                  </div>
                </th>
                <th className="py-3 px-3 cursor-pointer hover:text-[#EDF5F0]" onClick={() => handleSort('upload')}>
                  <div className="flex items-center space-x-1">
                    <span>Upload</span>
                    {sortField === 'upload' && (sortAsc ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)}
                  </div>
                </th>
                <th className="py-3 px-3 cursor-pointer hover:text-[#EDF5F0]" onClick={() => handleSort('total')}>
                  <div className="flex items-center space-x-1">
                    <span>Total Data</span>
                    {sortField === 'total' && (sortAsc ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)}
                  </div>
                </th>
                <th className="py-3 px-3 cursor-pointer hover:text-[#EDF5F0]" onClick={() => handleSort('connections')}>
                  <div className="flex items-center space-x-1">
                    <span>Sockets</span>
                    {sortField === 'connections' && (sortAsc ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)}
                  </div>
                </th>
                <th className="py-3 px-3">Status</th>
                <th className="py-3 px-4 text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[#192223]/60 font-sans">
              {filteredProcesses.length === 0 ? (
                <tr>
                  <td colSpan={8} className="py-12 text-center text-[#91A39A]">
                    No matching applications found.
                  </td>
                </tr>
              ) : (
                filteredProcesses.map((proc) => {
                  return (
                    <tr
                      key={`${proc.pid}-${proc.name}`}
                      className="hover:bg-[#192223]/40 transition-colors cursor-pointer group"
                      onClick={() => setSelectedProcess(proc)}
                    >
                      {/* Name & Exe */}
                      <td className="py-3 px-4">
                        <div className="flex items-center space-x-3">
                          <div className="w-8 h-8 rounded-lg bg-[#192223] border border-[#212D2E] flex items-center justify-center font-mono font-bold text-xs text-[#91DB79] uppercase shrink-0">
                            {proc.name.slice(0, 2)}
                          </div>
                          <div className="min-w-0">
                            <div className="font-semibold text-[#EDF5F0] truncate flex items-center gap-1.5">
                              {proc.name}
                              {proc.is_exclusive_target && (
                                <span className="text-[10px] uppercase font-mono px-1 py-0.2 rounded bg-[#EF6B70]/20 text-[#EF6B70] border border-[#EF6B70]/30 font-bold">
                                  EXCLUSIVE
                                </span>
                              )}
                            </div>
                            <div className="text-[11px] font-mono text-[#91A39A]/70 truncate max-w-xs" title={proc.exe_path}>
                              {truncateMiddle(proc.exe_path, 45)}
                            </div>
                          </div>
                        </div>
                      </td>

                      {/* PID */}
                      <td className="py-3 px-3 font-mono text-[#91A39A]">
                        {proc.pid}
                      </td>

                      {/* Download */}
                      <td className="py-3 px-3 font-mono">
                        <span className={proc.download_bytes_sec > 0 ? 'text-[#91DB79] font-semibold' : 'text-[#91A39A]/60'}>
                          {formatSpeed(proc.download_bytes_sec)}
                        </span>
                      </td>

                      {/* Upload */}
                      <td className="py-3 px-3 font-mono">
                        <span className={proc.upload_bytes_sec > 0 ? 'text-[#57C7B5] font-semibold' : 'text-[#91A39A]/60'}>
                          {formatSpeed(proc.upload_bytes_sec)}
                        </span>
                      </td>

                      {/* Total */}
                      <td className="py-3 px-3 font-mono text-[#EDF5F0]">
                        {formatBytes(proc.total_download_bytes + proc.total_upload_bytes)}
                      </td>

                      {/* Sockets */}
                      <td className="py-3 px-3 font-mono text-[#91A39A]">
                        {proc.connection_count}
                      </td>

                      {/* Access Status */}
                      <td className="py-3 px-3">
                        {proc.is_blocked ? (
                          <span className="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[11px] font-mono bg-[#EF6B70]/15 text-[#EF6B70] border border-[#EF6B70]/30">
                            <ShieldX className="w-3 h-3" />
                            <span>BLOCKED</span>
                          </span>
                        ) : proc.has_rules ? (
                          <span className="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[11px] font-mono bg-[#E9B65C]/15 text-[#E9B65C] border border-[#E9B65C]/30" title="Rate profile saved (packet-shaping unsupported in user-mode WFP ALE)">
                            <Sliders className="w-3 h-3" />
                            <span>LIMIT PROFILE (UNENFORCED)</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[11px] font-mono bg-[#91DB79]/15 text-[#91DB79] border border-[#91DB79]/30">
                            <Globe className="w-3 h-3" />
                            <span>ALLOWED</span>
                          </span>
                        )}
                      </td>

                      {/* Actions */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <div className="flex items-center justify-end space-x-2">
                          {/* Toggle Allow / Block */}
                          <button
                            onClick={() => onToggleBlock(proc)}
                            className={`p-1.5 rounded-lg border transition-colors ${
                              proc.is_blocked
                                ? 'bg-[#91DB79]/15 text-[#91DB79] border-[#91DB79]/40 hover:bg-[#91DB79]/25'
                                : 'bg-[#EF6B70]/15 text-[#EF6B70] border-[#EF6B70]/40 hover:bg-[#EF6B70]/25'
                            }`}
                            title={proc.is_blocked ? 'Unblock Internet' : 'Block Internet Access'}
                          >
                            {proc.is_blocked ? <ShieldCheck className="w-3.5 h-3.5" /> : <ShieldX className="w-3.5 h-3.5" />}
                          </button>

                          {/* Set Limit */}
                          <button
                            onClick={() => setLimitModalProc(proc)}
                            className="p-1.5 rounded-lg bg-[#192223] hover:bg-[#212D2E] text-[#91A39A] hover:text-[#EDF5F0] border border-[#212D2E] transition-colors"
                            title="Configure Bandwidth Limits"
                          >
                            <Gauge className="w-3.5 h-3.5" />
                          </button>

                          {/* Exclusive Mode Button */}
                          <button
                            onClick={() => onSelectExclusive(proc)}
                            className="px-2.5 py-1 rounded-lg bg-[#192223] hover:bg-[#91DB79]/20 hover:text-[#91DB79] text-[#EDF5F0] border border-[#212D2E] text-xs font-medium transition-colors flex items-center space-x-1"
                            title="Select for Exclusive Mode"
                          >
                            <Zap className="w-3 h-3" />
                            <span>Exclusive</span>
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
      {/* Rate Limit Modal */}
      {limitModalProc && (
        <div className="fixed inset-0 bg-[#0B0F10]/80 backdrop-blur-sm z-50 flex items-center justify-center p-4">
          <div className="bg-[#12191A] border border-[#192223] rounded-2xl w-full max-w-md p-6 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between pb-3 border-b border-[#192223]">
              <div className="flex items-center space-x-2">
                <Gauge className="w-5 h-5 text-[#91DB79]" />
                <h3 className="text-base font-bold text-[#EDF5F0]">Configure Bandwidth Limit</h3>
              </div>
              <button onClick={() => setLimitModalProc(null)} className="text-[#91A39A] hover:text-[#EDF5F0]">
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-3 text-xs">
              <div className="p-3 rounded-lg bg-[#0B0F10] border border-[#192223]">
                <div className="font-semibold text-sm text-[#EDF5F0]">{limitModalProc.name}</div>
                <div className="text-[11px] font-mono text-[#91A39A] truncate">{limitModalProc.exe_path}</div>
              </div>

              <div>
                <label className="block text-[#91A39A] mb-1">Limit Type</label>
                <select
                  value={limitType}
                  onChange={(e) => setLimitType(e.target.value as any)}
                  className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
                >
                  <option value="download_limit">Download Limit (Inbound)</option>
                  <option value="upload_limit">Upload Limit (Outbound)</option>
                  <option value="bidirectional_limit">Bidirectional Limit</option>
                </select>
              </div>

              <div>
                <label className="block text-[#91A39A] mb-1">Speed Cap (KB/s)</label>
                <input
                  type="number"
                  min="64"
                  step="64"
                  value={limitValueKbps}
                  onChange={(e) => setLimitValueKbps(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
                />
              </div>

              <div className="p-3 rounded-lg bg-[#E9B65C]/10 border border-[#E9B65C]/30 flex items-start space-x-2 text-[11px] text-[#E9B65C]">
                <Info className="w-4 h-4 shrink-0 mt-0.5" />
                <span>
                  <strong>Note on Bandwidth Shaping:</strong> Windows user-mode ALE layers enforce binary authorization (Allow/Block). Rate limits are saved as monitoring threshold profiles; active packet transmission pacing requires a kernel NDIS callout driver. Use <strong>Block</strong> or <strong>Exclusive Mode</strong> for active kernel-enforced restriction.
                </span>
              </div>

              <div className="pt-2 flex items-center justify-end space-x-3">
                <button
                  type="button"
                  onClick={() => setLimitModalProc(null)}
                  className="px-4 py-2 rounded-lg bg-[#192223] hover:bg-[#212D2E] text-[#EDF5F0]"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onSetLimit(limitModalProc);
                    setLimitModalProc(null);
                  }}
                  className="px-4 py-2 rounded-lg bg-[#E9B65C] hover:bg-[#E9B65C]/90 text-[#0B0F10] font-bold"
                >
                  Save Limit Profile (Informational)
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
