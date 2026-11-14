import React from 'react';
import { 
  ArrowDownCircle, 
  ArrowUpCircle, 
  HardDriveDownload, 
  HardDriveUpload, 
  Network, 
  AppWindow, 
  Play, 
  Pause, 
  Zap,
  ExternalLink
} from 'lucide-react';
import { 
  AreaChart, 
  Area, 
  XAxis, 
  YAxis, 
  Tooltip, 
  ResponsiveContainer 
} from 'recharts';
import { GlobalNetworkStats, ProcessNetworkInfo, TrafficHistoryPoint } from '../../types';
import { formatBytes, formatSpeed, truncateMiddle } from '../../utils/format';

interface OverviewPageProps {
  stats: GlobalNetworkStats;
  processes: ProcessNetworkInfo[];
  history: TrafficHistoryPoint[];
  isGraphPaused: boolean;
  onToggleGraphPause: () => void;
  onSelectExclusive: (process: ProcessNetworkInfo) => void;
  onNavigateToApplications: () => void;
}

export const OverviewPage: React.FC<OverviewPageProps> = ({
  stats,
  processes,
  history,
  isGraphPaused,
  onToggleGraphPause,
  onSelectExclusive,
  onNavigateToApplications,
}) => {
  // Sort top consumers by total current speed (download + upload)
  const topConsumers = [...processes]
    .sort((a, b) => (b.download_bytes_sec + b.upload_bytes_sec) - (a.download_bytes_sec + a.upload_bytes_sec))
    .slice(0, 5);

  return (
    <div className="space-y-6">
      {/* 1. Global Network Statistics Cards */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {/* Download Speed */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex flex-col justify-between">
          <div className="flex items-center justify-between text-[#91A39A] mb-2">
            <span className="text-xs font-medium">Download</span>
            <ArrowDownCircle className="w-4 h-4 text-[#91DB79]" />
          </div>
          <div>
            <div className="text-xl font-bold font-mono text-[#EDF5F0]">
              {formatSpeed(stats.download_speed_bps)}
            </div>
            <div className="text-[11px] text-[#91A39A] mt-0.5">Current Rate</div>
          </div>
        </div>

        {/* Upload Speed */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex flex-col justify-between">
          <div className="flex items-center justify-between text-[#91A39A] mb-2">
            <span className="text-xs font-medium">Upload</span>
            <ArrowUpCircle className="w-4 h-4 text-[#57C7B5]" />
          </div>
          <div>
            <div className="text-xl font-bold font-mono text-[#EDF5F0]">
              {formatSpeed(stats.upload_speed_bps)}
            </div>
            <div className="text-[11px] text-[#91A39A] mt-0.5">Current Rate</div>
          </div>
        </div>

        {/* Total Downloaded */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex flex-col justify-between">
          <div className="flex items-center justify-between text-[#91A39A] mb-2">
            <span className="text-xs font-medium">Total Inbound</span>
            <HardDriveDownload className="w-4 h-4 text-[#91DB79]/70" />
          </div>
          <div>
            <div className="text-xl font-bold font-mono text-[#EDF5F0]">
              {formatBytes(stats.total_download_bytes)}
            </div>
            <div className="text-[11px] text-[#91A39A] mt-0.5">Session Total</div>
          </div>
        </div>

        {/* Total Uploaded */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex flex-col justify-between">
          <div className="flex items-center justify-between text-[#91A39A] mb-2">
            <span className="text-xs font-medium">Total Outbound</span>
            <HardDriveUpload className="w-4 h-4 text-[#57C7B5]/70" />
          </div>
          <div>
            <div className="text-xl font-bold font-mono text-[#EDF5F0]">
              {formatBytes(stats.total_upload_bytes)}
            </div>
            <div className="text-[11px] text-[#91A39A] mt-0.5">Session Total</div>
          </div>
        </div>

        {/* Active Connections */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex flex-col justify-between">
          <div className="flex items-center justify-between text-[#91A39A] mb-2">
            <span className="text-xs font-medium">Connections</span>
            <Network className="w-4 h-4 text-[#91A39A]" />
          </div>
          <div>
            <div className="text-xl font-bold font-mono text-[#EDF5F0]">
              {stats.active_connections}
            </div>
            <div className="text-[11px] text-[#91A39A] mt-0.5">TCP/UDP Sockets</div>
          </div>
        </div>

        {/* Detected Applications */}
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223] flex flex-col justify-between">
          <div className="flex items-center justify-between text-[#91A39A] mb-2">
            <span className="text-xs font-medium">Active Apps</span>
            <AppWindow className="w-4 h-4 text-[#91A39A]" />
          </div>
          <div>
            <div className="text-xl font-bold font-mono text-[#EDF5F0]">
              {stats.detected_applications}
            </div>
            <div className="text-[11px] text-[#91A39A] mt-0.5">Network Processes</div>
          </div>
        </div>
      </div>

      {/* 2. Live Bandwidth Graph (60-second Rolling Telemetry) */}
      <div className="p-5 rounded-2xl bg-[#12191A] border border-[#192223] space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <h2 className="text-base font-semibold text-[#EDF5F0]">
              Real-Time Bandwidth Throughput
            </h2>
            <span className="text-xs font-mono px-2 py-0.5 rounded bg-[#192223] text-[#91A39A]">
              60s Window
            </span>
          </div>

          <div className="flex items-center space-x-4">
            {/* Legend */}
            <div className="flex items-center space-x-4 text-xs font-medium">
              <div className="flex items-center space-x-1.5">
                <span className="w-2.5 h-2.5 rounded-full bg-[#91DB79]"></span>
                <span className="text-[#91A39A]">Download</span>
              </div>
              <div className="flex items-center space-x-1.5">
                <span className="w-2.5 h-2.5 rounded-full bg-[#57C7B5]"></span>
                <span className="text-[#91A39A]">Upload</span>
              </div>
            </div>

            {/* Pause / Resume Control */}
            <button
              onClick={onToggleGraphPause}
              className="flex items-center space-x-1.5 px-2.5 py-1 rounded-lg bg-[#192223] hover:bg-[#212D2E] text-xs text-[#EDF5F0] transition-colors"
            >
              {isGraphPaused ? (
                <>
                  <Play className="w-3.5 h-3.5 text-[#91DB79]" />
                  <span>Resume</span>
                </>
              ) : (
                <>
                  <Pause className="w-3.5 h-3.5 text-[#E9B65C]" />
                  <span>Pause</span>
                </>
              )}
            </button>
          </div>
        </div>

        {/* Graph Container */}
        <div className="h-64 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={history} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
              <defs>
                <linearGradient id="downloadGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#91DB79" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="#91DB79" stopOpacity={0.0} />
                </linearGradient>
                <linearGradient id="uploadGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#57C7B5" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="#57C7B5" stopOpacity={0.0} />
                </linearGradient>
              </defs>
              <XAxis 
                dataKey="time" 
                stroke="#62736A" 
                fontSize={10} 
                tickLine={false} 
                interval="preserveStartEnd"
              />
              <YAxis 
                stroke="#62736A" 
                fontSize={10} 
                tickLine={false}
                tickFormatter={(val) => formatSpeed(val, 0)}
              />
              <Tooltip 
                content={({ active, payload }) => {
                  if (active && payload && payload.length) {
                    const data = payload[0].payload as TrafficHistoryPoint;
                    return (
                      <div className="bg-[#12191A] border border-[#192223] p-2.5 rounded-lg shadow-xl text-xs space-y-1 font-mono">
                        <div className="text-[#91A39A] font-sans">{data.time}</div>
                        <div className="text-[#91DB79]">
                          Down: {formatSpeed(data.rawDownload)}
                        </div>
                        <div className="text-[#57C7B5]">
                          Up: {formatSpeed(data.rawUpload)}
                        </div>
                      </div>
                    );
                  }
                  return null;
                }}
              />
              <Area
                type="monotone"
                dataKey="rawDownload"
                stroke="#91DB79"
                strokeWidth={2}
                fillOpacity={1}
                fill="url(#downloadGrad)"
                isAnimationActive={false}
              />
              <Area
                type="monotone"
                dataKey="rawUpload"
                stroke="#57C7B5"
                strokeWidth={2}
                fillOpacity={1}
                fill="url(#uploadGrad)"
                isAnimationActive={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* 3. Top Network Consumers */}
      <div className="p-5 rounded-2xl bg-[#12191A] border border-[#192223] space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-[#EDF5F0]">
              Top Network Consumers
            </h2>
            <p className="text-xs text-[#91A39A] mt-0.5">
              Processes actively transmitting or receiving data over the network
            </p>
          </div>

          <button
            onClick={onNavigateToApplications}
            className="flex items-center space-x-1 text-xs text-[#91DB79] hover:underline"
          >
            <span>View all ({processes.length})</span>
            <ExternalLink className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Consumer Process List */}
        <div className="divide-y divide-[#192223]">
          {topConsumers.length === 0 ? (
            <div className="py-8 text-center text-sm text-[#91A39A]">
              No active network traffic detected on currently monitored sockets.
            </div>
          ) : (
            topConsumers.map((proc) => {
              const totalSpeed = proc.download_bytes_sec + proc.upload_bytes_sec;
              return (
                <div
                  key={`${proc.pid}-${proc.name}`}
                  className="py-3 flex items-center justify-between hover:bg-[#192223]/30 px-2 rounded-lg transition-colors"
                >
                  <div className="flex items-center space-x-3.5 min-w-0 flex-1">
                    <div className="w-8 h-8 rounded-lg bg-[#192223] border border-[#212D2E] flex items-center justify-center text-xs font-mono font-bold text-[#57C7B5] uppercase">
                      {proc.name.slice(0, 2)}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center space-x-2">
                        <span className="font-semibold text-sm text-[#EDF5F0] truncate">
                          {proc.name}
                        </span>
                        <span className="text-[11px] font-mono px-1.5 py-0.2 rounded bg-[#192223] text-[#91A39A]">
                          PID: {proc.pid}
                        </span>
                        {proc.is_blocked && (
                          <span className="text-[10px] uppercase font-mono px-1.5 py-0.2 rounded bg-[#EF6B70]/20 text-[#EF6B70] border border-[#EF6B70]/30 font-bold">
                            BLOCKED
                          </span>
                        )}
                      </div>
                      <div className="text-[11px] font-mono text-[#91A39A]/70 truncate max-w-lg" title={proc.exe_path}>
                        {truncateMiddle(proc.exe_path, 60)}
                      </div>
                    </div>
                  </div>

                  {/* Speed indicators & Quick Actions */}
                  <div className="flex items-center space-x-6">
                    <div className="text-right font-mono">
                      <div className="text-xs font-semibold text-[#91DB79]">
                        ↓ {formatSpeed(proc.download_bytes_sec)}
                      </div>
                      <div className="text-[11px] text-[#57C7B5]">
                        ↑ {formatSpeed(proc.upload_bytes_sec)}
                      </div>
                    </div>

                    <div className="text-right font-mono hidden md:block">
                      <div className="text-xs text-[#EDF5F0]">
                        {formatBytes(proc.total_download_bytes + proc.total_upload_bytes)}
                      </div>
                      <div className="text-[11px] text-[#91A39A]">
                        {proc.connection_count} sockets
                      </div>
                    </div>

                    {/* Quick Exclusive Action */}
                    <button
                      onClick={() => onSelectExclusive(proc)}
                      className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-[#192223] hover:bg-[#91DB79]/20 hover:text-[#91DB79] text-xs font-medium text-[#EDF5F0] border border-[#212D2E] transition-all duration-150"
                      title="Select this application for Exclusive Mode"
                    >
                      <Zap className="w-3.5 h-3.5" />
                      <span className="hidden sm:inline">Exclusive</span>
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
};
