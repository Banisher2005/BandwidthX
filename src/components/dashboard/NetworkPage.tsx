import React, { useState, useEffect } from 'react';
import { 
  Play, 
  Square, 
  RotateCw, 
  ArrowDown, 
  ArrowUp, 
  Activity, 
  Wifi, 
  Clock, 
  Server,
  Zap,
  Globe
} from 'lucide-react';
import { 
  AreaChart, 
  Area, 
  XAxis, 
  YAxis, 
  Tooltip, 
  ResponsiveContainer 
} from 'recharts';
import { 
  GlobalNetworkStats, 
  TrafficHistoryPoint, 
  SpeedTestProgress 
} from '../../types';
import { api } from '../../services/api';
import { formatBytes, formatSpeed, formatBitsSpeedParts } from '../../utils/format';

interface NetworkPageProps {
  stats: GlobalNetworkStats;
  history: TrafficHistoryPoint[];
  speedTestEndpoint?: string;
}

export const NetworkPage: React.FC<NetworkPageProps> = ({
  stats,
  history,
  speedTestEndpoint,
}) => {
  const [speedTest, setSpeedTest] = useState<SpeedTestProgress>({
    is_running: false,
    phase: 'idle',
    progress_percent: 0,
    ping_ms: 0,
    jitter_ms: 0,
    download_speed_bps: 0,
    upload_speed_bps: 0,
    current_throughput_bps: 0,
    bytes_transferred: 0,
    server_name: 'Cloudflare Edge CDN (Default)',
    error_message: null,
    timestamp: null,
  });

  const [testActive, setTestActive] = useState(false);

  // Poll speed test progress when active
  useEffect(() => {
    let timer: ReturnType<typeof setInterval>;
    if (testActive) {
      timer = setInterval(async () => {
        try {
          const prog = await api.getSpeedTestProgress();
          setSpeedTest(prog);
          if (!prog.is_running && prog.phase !== 'latency' && prog.phase !== 'download' && prog.phase !== 'upload') {
            setTestActive(false);
          }
        } catch (e) {
          console.error('Failed to query speed test progress:', e);
        }
      }, 200);
    }
    return () => clearInterval(timer);
  }, [testActive]);

  const handleStartTest = async () => {
    setTestActive(true);
    try {
      const initial = await api.startSpeedTest(speedTestEndpoint);
      setSpeedTest(initial);
    } catch (err: any) {
      console.error('Speed test error:', err);
      setTestActive(false);
    }
  };

  const handleCancelTest = async () => {
    try {
      await api.cancelSpeedTest();
      setTestActive(false);
    } catch (err) {
      console.error('Failed to cancel speed test:', err);
    }
  };

  // Format dominant download display
  const isTesting = speedTest.is_running;
  const currentSpeedValue = isTesting && speedTest.phase === 'download'
    ? speedTest.current_throughput_bps
    : speedTest.download_speed_bps;

  const downloadParts = formatBitsSpeedParts(currentSpeedValue);
  const uploadParts = formatBitsSpeedParts(speedTest.upload_speed_bps);

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-10">
      {/* ----------------------------------------------------------------- */}
      {/* 1. CENTRAL SPEED-TEST PANEL (Fast.com-Style Hero Experience)      */}
      {/* ----------------------------------------------------------------- */}
      <div className="relative overflow-hidden rounded-3xl glass-panel-elevated p-8 md:p-12 border border-white/10 shadow-2xl text-center">
        {/* Subtle radial glow background */}
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-96 h-96 bg-accent-primary/10 rounded-full blur-3xl pointer-events-none"></div>

        {/* Header label & Endpoint info */}
        <div className="relative z-10 flex flex-col items-center justify-center space-y-1.5 mb-6">
          <div className="inline-flex items-center space-x-2 px-3 py-1 rounded-full bg-white/[0.05] border border-white/10 text-xs text-content-secondary">
            <Globe className="w-3.5 h-3.5 text-accent-secondary" />
            <span>{speedTest.server_name || 'Cloudflare Edge CDN'}</span>
            {speedTest.timestamp && (
              <span className="text-content-muted">
                • Tested {new Date(speedTest.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
              </span>
            )}
          </div>
          <h2 className="text-xs uppercase font-mono tracking-widest text-content-muted font-bold">
            Internet Throughput
          </h2>
        </div>

        {/* DOMINANT DOWNLOAD SPEED DISPLAY */}
        <div className="relative z-10 my-4 flex flex-col items-center justify-center">
          <div className="flex items-baseline justify-center space-x-3 select-none">
            <span className="text-7xl md:text-9xl font-black tracking-tighter text-content-primary transition-all duration-150">
              {downloadParts.value}
            </span>
            <span className="text-2xl md:text-4xl font-bold font-mono text-accent-primary">
              {downloadParts.unit}
            </span>
          </div>

          <div className="mt-1 text-xs font-mono text-content-muted uppercase tracking-wider">
            {isTesting ? (
              <span className="text-accent-secondary animate-pulse font-semibold">
                Measuring {speedTest.phase}...
              </span>
            ) : (
              <span>Download Speed</span>
            )}
          </div>
        </div>

        {/* Progress Bar (when testing) */}
        {isTesting && (
          <div className="relative z-10 w-full max-w-md mx-auto my-6">
            <div className="h-1.5 w-full bg-white/[0.08] rounded-full overflow-hidden">
              <div 
                className="h-full bg-gradient-to-r from-accent-primary to-accent-secondary transition-all duration-200 rounded-full"
                style={{ width: `${Math.max(5, speedTest.progress_percent)}%` }}
              ></div>
            </div>
            <div className="flex justify-between text-[11px] font-mono text-content-muted mt-2">
              <span className="capitalize">Phase: {speedTest.phase}</span>
              <span>{speedTest.progress_percent}%</span>
            </div>
          </div>
        )}

        {/* ERROR MESSAGE IF ANY */}
        {speedTest.error_message && (
          <div className="relative z-10 max-w-md mx-auto my-4 p-3 rounded-xl bg-status-danger/10 border border-status-danger/30 text-status-danger text-xs text-left">
            <p className="font-semibold mb-0.5">Speed Test Notice:</p>
            <p className="opacity-90">{speedTest.error_message}</p>
          </div>
        )}

        {/* SECONDARY METRICS: Upload, Latency, Jitter */}
        <div className="relative z-10 grid grid-cols-3 gap-4 max-w-lg mx-auto my-8">
          {/* Upload */}
          <div className="p-3.5 rounded-2xl glass-card flex flex-col items-center justify-center">
            <div className="flex items-center space-x-1 text-content-secondary text-[11px] font-medium mb-1">
              <ArrowUp className="w-3.5 h-3.5 text-accent-secondary" />
              <span>UPLOAD</span>
            </div>
            <div className="text-lg md:text-xl font-bold font-mono text-content-primary">
              {uploadParts.value} <span className="text-xs font-normal text-content-secondary">{uploadParts.unit}</span>
            </div>
          </div>

          {/* Latency */}
          <div className="p-3.5 rounded-2xl glass-card flex flex-col items-center justify-center">
            <div className="flex items-center space-x-1 text-content-secondary text-[11px] font-medium mb-1">
              <Activity className="w-3.5 h-3.5 text-accent-primary" />
              <span>LATENCY</span>
            </div>
            <div className="text-lg md:text-xl font-bold font-mono text-content-primary">
              {speedTest.ping_ms > 0 ? `${speedTest.ping_ms} ms` : '—'}
            </div>
          </div>

          {/* Jitter */}
          <div className="p-3.5 rounded-2xl glass-card flex flex-col items-center justify-center">
            <div className="flex items-center space-x-1 text-content-secondary text-[11px] font-medium mb-1">
              <Clock className="w-3.5 h-3.5 text-status-warning" />
              <span>JITTER</span>
            </div>
            <div className="text-lg md:text-xl font-bold font-mono text-content-primary">
              {speedTest.jitter_ms > 0 ? `${speedTest.jitter_ms} ms` : '—'}
            </div>
          </div>
        </div>

        {/* ACTION BUTTONS: Start, Cancel, Rerun */}
        <div className="relative z-10 flex items-center justify-center space-x-4">
          {!isTesting ? (
            <button
              onClick={handleStartTest}
              className="flex items-center space-x-2 px-8 py-3.5 rounded-2xl bg-gradient-to-r from-accent-primary to-accent-secondary text-background font-bold text-sm shadow-lg shadow-accent-primary/20 hover:opacity-95 transition-all transform hover:scale-[1.02] active:scale-[0.98]"
            >
              {speedTest.phase === 'completed' ? (
                <>
                  <RotateCw className="w-4 h-4" />
                  <span>Rerun Speed Test</span>
                </>
              ) : (
                <>
                  <Play className="w-4 h-4 fill-background" />
                  <span>Start Speed Test</span>
                </>
              )}
            </button>
          ) : (
            <button
              onClick={handleCancelTest}
              className="flex items-center space-x-2 px-8 py-3.5 rounded-2xl bg-status-danger/20 hover:bg-status-danger/30 border border-status-danger/40 text-status-danger font-semibold text-sm transition-all"
            >
              <Square className="w-4 h-4 fill-status-danger" />
              <span>Cancel Test</span>
            </button>
          )}
        </div>
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* 2. ALWAYS-ON REAL-TIME NETWORK MONITOR (Independent of Speed Test) */}
      {/* ----------------------------------------------------------------- */}
      <div className="rounded-3xl glass-card p-6 md:p-8 space-y-6">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <div className="flex items-center space-x-2">
              <span className="w-2.5 h-2.5 rounded-full bg-accent-primary animate-pulse"></span>
              <h3 className="text-base font-semibold text-content-primary">Live Adapter Telemetry</h3>
            </div>
            <p className="text-xs text-content-secondary mt-0.5">
              Continuous live throughput from Windows IP Helper ({stats.adapter_name})
            </p>
          </div>
          <div className="flex items-center space-x-3 text-xs font-mono text-content-secondary">
            <span className="px-2.5 py-1 rounded-xl bg-white/[0.04] border border-white/[0.08]">
              {stats.active_connections} Sockets
            </span>
            <span className="px-2.5 py-1 rounded-xl bg-white/[0.04] border border-white/[0.08]">
              {stats.detected_applications} Active Apps
            </span>
          </div>
        </div>

        {/* 4 Primary Stats Cards */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          {/* Current Download Rate */}
          <div className="p-4 rounded-2xl bg-white/[0.025] border border-white/[0.06]">
            <div className="flex items-center justify-between text-xs text-content-secondary mb-1.5">
              <span className="font-medium">Current In</span>
              <ArrowDown className="w-3.5 h-3.5 text-accent-primary" />
            </div>
            <div className="text-xl md:text-2xl font-bold font-mono text-accent-primary">
              {formatSpeed(stats.download_speed_bps)}
            </div>
          </div>

          {/* Current Upload Rate */}
          <div className="p-4 rounded-2xl bg-white/[0.025] border border-white/[0.06]">
            <div className="flex items-center justify-between text-xs text-content-secondary mb-1.5">
              <span className="font-medium">Current Out</span>
              <ArrowUp className="w-3.5 h-3.5 text-accent-secondary" />
            </div>
            <div className="text-xl md:text-2xl font-bold font-mono text-accent-secondary">
              {formatSpeed(stats.upload_speed_bps)}
            </div>
          </div>

          {/* Total Downloaded */}
          <div className="p-4 rounded-2xl bg-white/[0.025] border border-white/[0.06]">
            <div className="flex items-center justify-between text-xs text-content-secondary mb-1.5">
              <span className="font-medium">Total Inbound</span>
              <Zap className="w-3.5 h-3.5 text-content-muted" />
            </div>
            <div className="text-xl md:text-2xl font-bold font-mono text-content-primary">
              {formatBytes(stats.total_download_bytes)}
            </div>
          </div>

          {/* Total Uploaded */}
          <div className="p-4 rounded-2xl bg-white/[0.025] border border-white/[0.06]">
            <div className="flex items-center justify-between text-xs text-content-secondary mb-1.5">
              <span className="font-medium">Total Outbound</span>
              <Zap className="w-3.5 h-3.5 text-content-muted" />
            </div>
            <div className="text-xl md:text-2xl font-bold font-mono text-content-primary">
              {formatBytes(stats.total_upload_bytes)}
            </div>
          </div>
        </div>

        {/* Rolling 60s Telemetry Waveform */}
        <div className="pt-2">
          <div className="flex items-center justify-between text-xs text-content-muted mb-2 font-mono">
            <span>60-Second Real-Time Network Activity</span>
            <div className="flex items-center space-x-4">
              <span className="flex items-center space-x-1.5">
                <span className="w-2.5 h-2.5 rounded-full bg-accent-primary"></span>
                <span>Download</span>
              </span>
              <span className="flex items-center space-x-1.5">
                <span className="w-2.5 h-2.5 rounded-full bg-accent-secondary"></span>
                <span>Upload</span>
              </span>
            </div>
          </div>

          <div className="h-44 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={history} margin={{ top: 5, right: 0, left: -20, bottom: 0 }}>
                <defs>
                  <linearGradient id="downloadGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#91DB79" stopOpacity={0.25} />
                    <stop offset="95%" stopColor="#91DB79" stopOpacity={0} />
                  </linearGradient>
                  <linearGradient id="uploadGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#57C7B5" stopOpacity={0.25} />
                    <stop offset="95%" stopColor="#57C7B5" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis 
                  dataKey="time" 
                  stroke="#5F6F7E" 
                  fontSize={10} 
                  tickLine={false} 
                  axisLine={false} 
                  interval={14}
                />
                <YAxis 
                  stroke="#5F6F7E" 
                  fontSize={10} 
                  tickLine={false} 
                  axisLine={false}
                  tickFormatter={(val) => formatSpeed(val, 0)}
                />
                <Tooltip 
                  contentStyle={{ 
                    backgroundColor: '#101820', 
                    borderColor: 'rgba(255,255,255,0.1)', 
                    borderRadius: '1rem',
                    fontSize: '11px',
                    fontFamily: 'monospace',
                  }}
                  formatter={(value: any) => [formatSpeed(Number(value) || 0), 'Throughput']}
                />
                <Area 
                  type="monotone" 
                  dataKey="download" 
                  stroke="#91DB79" 
                  strokeWidth={2}
                  fillOpacity={1} 
                  fill="url(#downloadGrad)" 
                  isAnimationActive={false}
                />
                <Area 
                  type="monotone" 
                  dataKey="upload" 
                  stroke="#57C7B5" 
                  strokeWidth={1.5}
                  fillOpacity={1} 
                  fill="url(#uploadGrad)" 
                  isAnimationActive={false}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>
    </div>
  );
};
