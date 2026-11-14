import React, { useState } from 'react';
import { 
  Download, 
  Calendar, 
  ArrowDownCircle, 
  ArrowUpCircle, 
  Filter, 
  PieChart as PieIcon, 
  BarChart2,
  FileSpreadsheet
} from 'lucide-react';
import { 
  BarChart, 
  Bar, 
  XAxis, 
  YAxis, 
  Tooltip, 
  ResponsiveContainer, 
  PieChart, 
  Pie, 
  Cell 
} from 'recharts';
import { AnalyticsRecord } from '../../types';
import { formatBytes } from '../../utils/format';

interface AnalyticsPageProps {
  analytics: AnalyticsRecord[];
  onExportCsv: (range: string) => void;
}

export const AnalyticsPage: React.FC<AnalyticsPageProps> = ({
  analytics,
  onExportCsv,
}) => {
  const [timeRange, setTimeRange] = useState<'hourly' | 'daily' | 'weekly' | 'monthly'>('hourly');

  // Compute totals
  const totalDownload = analytics.reduce((acc, curr) => acc + curr.bytes_downloaded, 0);
  const totalUpload = analytics.reduce((acc, curr) => acc + curr.bytes_uploaded, 0);
  const totalBandwidth = totalDownload + totalUpload;

  // Chart data for applications
  const appData = analytics.slice(0, 7).map((item) => ({
    name: item.app_name,
    download: item.bytes_downloaded,
    upload: item.bytes_uploaded,
    total: item.bytes_downloaded + item.bytes_uploaded,
  }));

  const pieData = [
    { name: 'Inbound Traffic', value: totalDownload || 1, color: '#91DB79' },
    { name: 'Outbound Traffic', value: totalUpload || 1, color: '#57C7B5' },
  ];

  return (
    <div className="space-y-6">
      {/* 1. Header with Range Picker & CSV Export */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 p-4 rounded-xl bg-[#12191A] border border-[#192223]">
        <div className="flex items-center space-x-2">
          <Calendar className="w-4 h-4 text-[#91DB79]" />
          <span className="text-sm font-semibold text-[#EDF5F0]">Telemetry Interval:</span>
          <div className="flex items-center space-x-1">
            {[
              { id: 'hourly', label: 'Last 24 Hours' },
              { id: 'daily', label: 'Last 7 Days' },
              { id: 'weekly', label: 'Last 30 Days' },
              { id: 'monthly', label: 'All-Time' },
            ].map((tab) => (
              <button
                key={tab.id}
                onClick={() => setTimeRange(tab.id as any)}
                className={`px-3 py-1 rounded-lg text-xs font-medium transition-colors ${
                  timeRange === tab.id
                    ? 'bg-[#192223] text-[#91DB79] border border-[#91DB79]/30'
                    : 'text-[#91A39A] hover:bg-[#192223]/50 hover:text-[#EDF5F0]'
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {/* Export to CSV */}
        <button
          onClick={() => onExportCsv(timeRange)}
          className="flex items-center space-x-2 px-3.5 py-1.5 rounded-lg bg-[#192223] hover:bg-[#91DB79]/20 hover:text-[#91DB79] text-[#EDF5F0] border border-[#212D2E] text-xs font-medium transition-colors"
          title="Export current analytics dataset as CSV"
        >
          <FileSpreadsheet className="w-4 h-4" />
          <span>Export to CSV</span>
        </button>
      </div>

      {/* 2. Summary Metric Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223]">
          <div className="flex items-center justify-between text-[#91A39A] mb-1">
            <span className="text-xs font-medium">Total Downloaded</span>
            <ArrowDownCircle className="w-4 h-4 text-[#91DB79]" />
          </div>
          <div className="text-2xl font-bold font-mono text-[#EDF5F0]">
            {formatBytes(totalDownload)}
          </div>
          <div className="text-[11px] text-[#91A39A] mt-1">Inbound bytes recorded</div>
        </div>

        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223]">
          <div className="flex items-center justify-between text-[#91A39A] mb-1">
            <span className="text-xs font-medium">Total Uploaded</span>
            <ArrowUpCircle className="w-4 h-4 text-[#57C7B5]" />
          </div>
          <div className="text-2xl font-bold font-mono text-[#EDF5F0]">
            {formatBytes(totalUpload)}
          </div>
          <div className="text-[11px] text-[#91A39A] mt-1">Outbound bytes recorded</div>
        </div>

        <div className="p-4 rounded-xl bg-[#12191A] border border-[#192223]">
          <div className="flex items-center justify-between text-[#91A39A] mb-1">
            <span className="text-xs font-medium">Total Aggregated Data</span>
            <BarChart2 className="w-4 h-4 text-[#EDF5F0]" />
          </div>
          <div className="text-2xl font-bold font-mono text-[#EDF5F0]">
            {formatBytes(totalBandwidth)}
          </div>
          <div className="text-[11px] text-[#91A39A] mt-1">Combined throughput</div>
        </div>
      </div>

      {/* 3. Charts: App-wise breakdown & Direction distribution */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Bar Chart: Applications */}
        <div className="lg:col-span-2 p-5 rounded-2xl bg-[#12191A] border border-[#192223] space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-[#EDF5F0]">
              Application Bandwidth Distribution
            </h3>
            <span className="text-xs text-[#91A39A]">Top Consumers</span>
          </div>

          <div className="h-64 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={appData} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                <XAxis dataKey="name" stroke="#62736A" fontSize={10} tickLine={false} />
                <YAxis 
                  stroke="#62736A" 
                  fontSize={10} 
                  tickLine={false} 
                  tickFormatter={(val) => formatBytes(val, 0)} 
                />
                <Tooltip 
                  formatter={(val: number) => formatBytes(val)}
                  contentStyle={{ backgroundColor: '#12191A', borderColor: '#192223', borderRadius: '8px', fontSize: '12px' }}
                />
                <Bar dataKey="download" name="Download" fill="#91DB79" radius={[4, 4, 0, 0]} />
                <Bar dataKey="upload" name="Upload" fill="#57C7B5" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        {/* Pie Chart: Inbound vs Outbound Ratio */}
        <div className="p-5 rounded-2xl bg-[#12191A] border border-[#192223] space-y-4 flex flex-col justify-between">
          <h3 className="text-sm font-semibold text-[#EDF5F0]">
            Traffic Direction Ratio
          </h3>

          <div className="h-48 w-full flex items-center justify-center">
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie
                  data={pieData}
                  cx="50%"
                  cy="50%"
                  innerRadius={50}
                  outerRadius={75}
                  paddingAngle={4}
                  dataKey="value"
                >
                  {pieData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Pie>
                <Tooltip 
                  formatter={(val: number) => formatBytes(val)}
                  contentStyle={{ backgroundColor: '#12191A', borderColor: '#192223', borderRadius: '8px', fontSize: '12px' }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>

          <div className="space-y-2 pt-2 border-t border-[#192223] text-xs">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-[#91DB79]"></span>
                <span className="text-[#91A39A]">Inbound (Down)</span>
              </div>
              <span className="font-mono text-[#EDF5F0]">
                {totalBandwidth > 0 ? ((totalDownload / totalBandwidth) * 100).toFixed(1) : 0}%
              </span>
            </div>
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="w-2.5 h-2.5 rounded-full bg-[#57C7B5]"></span>
                <span className="text-[#91A39A]">Outbound (Up)</span>
              </div>
              <span className="font-mono text-[#EDF5F0]">
                {totalBandwidth > 0 ? ((totalUpload / totalBandwidth) * 100).toFixed(1) : 0}%
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* 4. Detailed History Table */}
      <div className="rounded-xl bg-[#12191A] border border-[#192223] overflow-hidden">
        <div className="p-4 border-b border-[#192223]">
          <h3 className="text-sm font-semibold text-[#EDF5F0]">
            Historical Usage Ledger
          </h3>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-[#0B0F10]/60 border-b border-[#192223] text-[#91A39A] font-mono">
              <tr>
                <th className="py-2.5 px-4">Application</th>
                <th className="py-2.5 px-3">Path</th>
                <th className="py-2.5 px-3">Downloaded</th>
                <th className="py-2.5 px-3">Uploaded</th>
                <th className="py-2.5 px-3">Total Volume</th>
                <th className="py-2.5 px-4 text-right">Recorded Timestamp</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[#192223]/60 font-sans">
              {analytics.length === 0 ? (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-[#91A39A]">
                    No historical records accumulated yet. Data is logged locally every minute.
                  </td>
                </tr>
              ) : (
                analytics.map((item, idx) => (
                  <tr key={idx} className="hover:bg-[#192223]/30">
                    <td className="py-2.5 px-4 font-semibold text-[#EDF5F0]">
                      {item.app_name}
                    </td>
                    <td className="py-2.5 px-3 font-mono text-[#91A39A] max-w-xs truncate" title={item.executable_path}>
                      {item.executable_path}
                    </td>
                    <td className="py-2.5 px-3 font-mono text-[#91DB79]">
                      {formatBytes(item.bytes_downloaded)}
                    </td>
                    <td className="py-2.5 px-3 font-mono text-[#57C7B5]">
                      {formatBytes(item.bytes_uploaded)}
                    </td>
                    <td className="py-2.5 px-3 font-mono text-[#EDF5F0]">
                      {formatBytes(item.bytes_downloaded + item.bytes_uploaded)}
                    </td>
                    <td className="py-2.5 px-4 text-right font-mono text-[#91A39A]">
                      {item.timestamp}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
