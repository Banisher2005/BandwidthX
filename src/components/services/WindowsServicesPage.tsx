import React, { useState, useEffect } from 'react';
import { 
  Server, 
  Search, 
  ShieldAlert, 
  ShieldCheck, 
  Play, 
  Square, 
  RotateCw, 
  AlertTriangle, 
  Info, 
  Cpu, 
  Folder, 
  CheckCircle2, 
  XCircle,
  Filter,
  Lock
} from 'lucide-react';
import { WindowsServiceInfo, ServiceSafetyClassification } from '../../types';
import { api } from '../../services/api';

interface WindowsServicesPageProps {
  onNotification: (msg: string) => void;
}

export const WindowsServicesPage: React.FC<WindowsServicesPageProps> = ({
  onNotification,
}) => {
  const [services, setServices] = useState<WindowsServiceInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [filterType, setFilterType] = useState<'all' | 'running' | 'protected' | 'system' | 'user'>('all');
  const [selectedService, setSelectedService] = useState<WindowsServiceInfo | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  const fetchServices = async () => {
    try {
      setLoading(true);
      const list = await api.getWindowsServicesList();
      setServices(list);
    } catch (err: any) {
      console.error('Failed to load Windows services:', err);
      onNotification(`Error loading services: ${err?.message || err}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchServices();
  }, []);

  const handleStart = async (service: WindowsServiceInfo) => {
    try {
      setActionLoading(service.service_name);
      await api.startWindowsService(service.service_name);
      onNotification(`Started service: ${service.display_name}`);
      await fetchServices();
    } catch (err: any) {
      onNotification(`Failed to start ${service.display_name}: ${err?.message || err}`);
    } finally {
      setActionLoading(null);
    }
  };

  const handleStop = async (service: WindowsServiceInfo) => {
    if (service.classification === 'Protected') {
      onNotification(`Action Denied: ${service.display_name} is a protected core Windows service`);
      return;
    }

    if (service.classification === 'System') {
      const confirm = window.confirm(
        `Warning: "${service.display_name}" is a Windows System Service. Stopping it may affect OS components. Are you sure you want to proceed?`
      );
      if (!confirm) return;
    }

    try {
      setActionLoading(service.service_name);
      await api.stopWindowsService(service.service_name);
      onNotification(`Stopped service: ${service.display_name}`);
      await fetchServices();
    } catch (err: any) {
      onNotification(`Failed to stop ${service.display_name}: ${err?.message || err}`);
    } finally {
      setActionLoading(null);
    }
  };

  const handleRestart = async (service: WindowsServiceInfo) => {
    if (service.classification === 'Protected') {
      onNotification(`Action Denied: ${service.display_name} is protected`);
      return;
    }

    try {
      setActionLoading(service.service_name);
      await api.restartWindowsService(service.service_name);
      onNotification(`Restarted service: ${service.display_name}`);
      await fetchServices();
    } catch (err: any) {
      onNotification(`Failed to restart ${service.display_name}: ${err?.message || err}`);
    } finally {
      setActionLoading(null);
    }
  };

  // Filter & Search Logic
  const filtered = services.filter((s) => {
    const matchesSearch = 
      s.service_name.toLowerCase().includes(search.toLowerCase()) ||
      s.display_name.toLowerCase().includes(search.toLowerCase()) ||
      (s.pid !== null && s.pid.toString().includes(search));

    if (!matchesSearch) return false;

    if (filterType === 'running') return s.status === 'Running';
    if (filterType === 'protected') return s.classification === 'Protected';
    if (filterType === 'system') return s.classification === 'System';
    if (filterType === 'user') return s.classification === 'User';
    return true;
  });

  const getSafetyBadge = (c: ServiceSafetyClassification) => {
    switch (c) {
      case 'Protected':
        return (
          <span className="inline-flex items-center space-x-1 px-2.5 py-0.5 rounded-full text-[10px] font-mono font-semibold bg-purple-500/15 text-purple-300 border border-purple-500/30">
            <Lock className="w-2.5 h-2.5" />
            <span>PROTECTED</span>
          </span>
        );
      case 'System':
        return (
          <span className="inline-flex items-center space-x-1 px-2.5 py-0.5 rounded-full text-[10px] font-mono font-semibold bg-status-warning/15 text-status-warning border border-status-warning/30">
            <ShieldAlert className="w-2.5 h-2.5" />
            <span>SYSTEM</span>
          </span>
        );
      case 'User':
        return (
          <span className="inline-flex items-center space-x-1 px-2.5 py-0.5 rounded-full text-[10px] font-mono font-semibold bg-accent-primary/15 text-accent-primary border border-accent-primary/30">
            <CheckCircle2 className="w-2.5 h-2.5" />
            <span>THIRD-PARTY</span>
          </span>
        );
      default:
        return (
          <span className="inline-flex items-center space-x-1 px-2.5 py-0.5 rounded-full text-[10px] font-mono text-content-muted bg-white/[0.04] border border-white/[0.08]">
            <span>UNKNOWN</span>
          </span>
        );
    }
  };

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-10">
      {/* Page Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center space-x-2.5">
            <div className="p-2 rounded-xl bg-purple-500/15 border border-purple-500/30 text-purple-400">
              <Server className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-xl font-bold text-content-primary tracking-tight">Windows Services</h2>
              <p className="text-xs text-content-secondary mt-0.5">
                Native Service Control Manager (SCM) inventory with automated safety classification.
              </p>
            </div>
          </div>
        </div>

        {/* Safety Legend */}
        <div className="flex items-center space-x-2 text-xs">
          <span className="px-2.5 py-1 rounded-xl glass-card text-content-secondary flex items-center space-x-1.5">
            <Lock className="w-3 h-3 text-purple-400" />
            <span>Core OS Locked</span>
          </span>
          <span className="px-2.5 py-1 rounded-xl glass-card text-content-secondary flex items-center space-x-1.5">
            <ShieldAlert className="w-3 h-3 text-status-warning" />
            <span>Warn on Stop</span>
          </span>
          <button 
            onClick={fetchServices}
            className="p-1.5 rounded-xl glass-card text-content-secondary hover:text-content-primary glass-card-hover"
            title="Reload SCM Services"
          >
            <RotateCw className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Search & Filters */}
      <div className="flex flex-col sm:flex-row gap-3">
        <div className="relative flex-1">
          <Search className="w-4 h-4 text-content-muted absolute left-3.5 top-1/2 -translate-y-1/2" />
          <input 
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search service by name, display name, or PID..."
            className="w-full pl-10 pr-4 py-2.5 rounded-2xl glass-card bg-white/[0.03] text-sm text-content-primary placeholder:text-content-muted focus:outline-none focus:border-accent-primary/50 transition-colors"
          />
        </div>

        <div className="flex items-center space-x-1.5 p-1 rounded-2xl glass-card shrink-0 text-xs">
          {(['all', 'running', 'protected', 'system', 'user'] as const).map((t) => (
            <button
              key={t}
              onClick={() => setFilterType(t)}
              className={`px-3 py-1.5 rounded-xl font-medium capitalize transition-all ${
                filterType === t 
                  ? 'bg-accent-primary text-background font-bold shadow-md' 
                  : 'text-content-secondary hover:text-content-primary'
              }`}
            >
              {t}
            </button>
          ))}
        </div>
      </div>

      {/* Services Table */}
      <div className="rounded-3xl glass-card overflow-hidden">
        {loading ? (
          <div className="py-20 text-center text-content-muted text-sm flex flex-col items-center justify-center space-y-2">
            <RotateCw className="w-6 h-6 animate-spin text-accent-primary" />
            <span>Querying Windows Service Control Manager...</span>
          </div>
        ) : filtered.length === 0 ? (
          <div className="py-16 text-center text-content-muted text-sm">
            No Windows services match your criteria.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs border-collapse">
              <thead>
                <tr className="border-b border-white/[0.06] text-content-muted uppercase tracking-wider font-mono text-[10px]">
                  <th className="py-3.5 px-4 font-semibold">Service</th>
                  <th className="py-3.5 px-3 font-semibold">PID</th>
                  <th className="py-3.5 px-3 font-semibold">Classification</th>
                  <th className="py-3.5 px-3 font-semibold">Status</th>
                  <th className="py-3.5 px-3 font-semibold">Startup</th>
                  <th className="py-3.5 px-4 font-semibold text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/[0.04]">
                {filtered.map((s) => {
                  const isRunning = s.status === 'Running';
                  const isProtected = s.classification === 'Protected';
                  const isActioning = actionLoading === s.service_name;

                  return (
                    <tr 
                      key={s.service_name} 
                      className="hover:bg-white/[0.025] transition-colors group cursor-pointer"
                      onClick={() => setSelectedService(s)}
                    >
                      {/* Name & Display */}
                      <td className="py-3 px-4 max-w-xs">
                        <div className="font-semibold text-content-primary truncate" title={s.display_name}>
                          {s.display_name}
                        </div>
                        <div className="text-[11px] font-mono text-content-muted truncate" title={s.service_name}>
                          {s.service_name}
                        </div>
                      </td>

                      {/* PID */}
                      <td className="py-3 px-3 font-mono text-content-secondary">
                        {s.pid ? (
                          <span className="px-1.5 py-0.5 rounded bg-white/[0.04] border border-white/[0.08]">
                            {s.pid}
                          </span>
                        ) : (
                          <span className="text-content-muted">—</span>
                        )}
                      </td>

                      {/* Safety Classification */}
                      <td className="py-3 px-3">
                        {getSafetyBadge(s.classification)}
                      </td>

                      {/* Status */}
                      <td className="py-3 px-3">
                        <span className={`inline-flex items-center space-x-1.5 font-medium ${
                          isRunning ? 'text-accent-primary' : 'text-content-muted'
                        }`}>
                          <span className={`w-1.5 h-1.5 rounded-full ${isRunning ? 'bg-accent-primary animate-pulse' : 'bg-content-muted'}`}></span>
                          <span>{s.status}</span>
                        </span>
                      </td>

                      {/* Startup Type */}
                      <td className="py-3 px-3 text-content-secondary">
                        {s.startup_type}
                      </td>

                      {/* Action Controls */}
                      <td className="py-3 px-4 text-right" onClick={(e) => e.stopPropagation()}>
                        <div className="flex items-center justify-end space-x-1.5">
                          {isProtected ? (
                            <span 
                              className="text-[11px] text-content-muted flex items-center space-x-1 px-2.5 py-1 rounded-xl bg-white/[0.02] border border-white/[0.06]" 
                              title="Core Windows service protected against termination"
                            >
                              <Lock className="w-3 h-3 text-purple-400" />
                              <span>Protected</span>
                            </span>
                          ) : (
                            <>
                              {/* Start */}
                              {s.can_start && (
                                <button
                                  onClick={() => handleStart(s)}
                                  disabled={isActioning}
                                  className="p-1.5 rounded-xl bg-accent-primary/10 hover:bg-accent-primary/20 text-accent-primary border border-accent-primary/30 transition-all disabled:opacity-50"
                                  title="Start Service"
                                >
                                  <Play className="w-3.5 h-3.5 fill-accent-primary" />
                                </button>
                              )}

                              {/* Stop */}
                              {s.can_stop && (
                                <button
                                  onClick={() => handleStop(s)}
                                  disabled={isActioning}
                                  className="p-1.5 rounded-xl bg-status-danger/10 hover:bg-status-danger/20 text-status-danger border border-status-danger/30 transition-all disabled:opacity-50"
                                  title="Stop Service"
                                >
                                  <Square className="w-3.5 h-3.5 fill-status-danger" />
                                </button>
                              )}

                              {/* Restart */}
                              {s.can_restart && (
                                <button
                                  onClick={() => handleRestart(s)}
                                  disabled={isActioning}
                                  className="p-1.5 rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-content-secondary hover:text-content-primary border border-white/[0.08] transition-all disabled:opacity-50"
                                  title="Restart Service"
                                >
                                  <RotateCw className="w-3.5 h-3.5" />
                                </button>
                              )}
                            </>
                          )}
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

      {/* Service Details Drawer / Modal */}
      {selectedService && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-background/80 backdrop-blur-md">
          <div className="max-w-lg w-full rounded-3xl glass-panel-elevated p-6 border border-white/10 shadow-2xl space-y-5 animate-in fade-in zoom-in-95 duration-150">
            <div className="flex items-start justify-between">
              <div>
                <div className="flex items-center space-x-2">
                  <Server className="w-4 h-4 text-accent-secondary" />
                  <h3 className="text-base font-bold text-content-primary tracking-tight">
                    {selectedService.display_name}
                  </h3>
                </div>
                <div className="text-xs font-mono text-content-muted mt-0.5">
                  Service Name: {selectedService.service_name}
                </div>
              </div>
              <button 
                onClick={() => setSelectedService(null)}
                className="p-1 rounded-xl text-content-muted hover:text-content-primary glass-card"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3 text-xs">
              <div className="p-3 rounded-2xl bg-white/[0.025] border border-white/[0.06] space-y-2">
                <div className="flex justify-between">
                  <span className="text-content-secondary">Safety Level</span>
                  <span>{getSafetyBadge(selectedService.classification)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">Current State</span>
                  <span className="font-semibold text-content-primary">{selectedService.status}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">Process ID (PID)</span>
                  <span className="font-mono text-content-primary">{selectedService.pid || 'Not running'}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-content-secondary">Startup Type</span>
                  <span className="text-content-primary">{selectedService.startup_type}</span>
                </div>
              </div>

              {selectedService.exe_path && (
                <div className="p-3 rounded-2xl bg-white/[0.025] border border-white/[0.06] space-y-1">
                  <span className="text-content-secondary block text-[11px]">Binary Path</span>
                  <span className="font-mono text-[11px] text-content-primary break-all select-all block">
                    {selectedService.exe_path}
                  </span>
                </div>
              )}

              {selectedService.classification === 'Protected' && (
                <div className="p-3 rounded-2xl bg-purple-500/10 border border-purple-500/20 text-purple-300 text-[11px] leading-relaxed">
                  🛡️ This service is vital to Windows operational stability, security authentication, or packet filtering. BandwidthX protects it against modification or termination.
                </div>
              )}
            </div>

            <div className="flex justify-end pt-2">
              <button
                onClick={() => setSelectedService(null)}
                className="px-5 py-2 rounded-xl glass-card text-content-primary font-medium text-xs hover:bg-white/[0.08]"
              >
                Close Details
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
