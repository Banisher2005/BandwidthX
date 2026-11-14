import React, { useState } from 'react';
import { 
  Plus, 
  ShieldCheck, 
  ShieldX, 
  Gauge, 
  Star, 
  Trash2, 
  ToggleLeft, 
  ToggleRight, 
  SlidersHorizontal,
  X
} from 'lucide-react';
import { NetworkRule, RuleType, Direction } from '../../types';
import { formatSpeed, truncateMiddle } from '../../utils/format';

interface RulesPageProps {
  rules: NetworkRule[];
  onSaveRule: (rule: NetworkRule) => void;
  onDeleteRule: (ruleId: string, exePath: string) => void;
  onEmergencyRestore: () => void;
}

export const RulesPage: React.FC<RulesPageProps> = ({
  rules,
  onSaveRule,
  onDeleteRule,
  onEmergencyRestore,
}) => {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [appName, setAppName] = useState('');
  const [exePath, setExePath] = useState('');
  const [ruleType, setRuleType] = useState<RuleType>('block');
  const [direction, setDirection] = useState<Direction>('both');
  const [limitKbps, setLimitKbps] = useState('1024');

  const handleCreateRule = (e: React.FormEvent) => {
    e.preventDefault();
    if (!exePath.trim()) return;

    const newRule: NetworkRule = {
      id: `rule-${Date.now()}`,
      app_name: appName.trim() || exePath.split(/[/\\]/).pop() || 'Application',
      executable_path: exePath.trim(),
      rule_type: ruleType,
      direction: direction,
      limit_bytes_per_sec: ruleType.includes('limit') ? parseInt(limitKbps, 10) * 1024 : null,
      enabled: true,
      created_at: new Date().toISOString().replace('T', ' ').slice(0, 19),
      updated_at: new Date().toISOString().replace('T', ' ').slice(0, 19),
    };

    onSaveRule(newRule);
    setIsModalOpen(false);
    setAppName('');
    setExePath('');
  };

  const handleToggleRule = (rule: NetworkRule) => {
    const updated = { ...rule, enabled: !rule.enabled, updated_at: new Date().toISOString().replace('T', ' ').slice(0, 19) };
    onSaveRule(updated);
  };

  return (
    <div className="space-y-4">
      {/* Action Header */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 p-4 rounded-xl bg-[#12191A] border border-[#192223]">
        <div>
          <h2 className="text-base font-semibold text-[#EDF5F0] flex items-center gap-2">
            <SlidersHorizontal className="w-4 h-4 text-[#91DB79]" />
            Network Rule Engine
          </h2>
          <p className="text-xs text-[#91A39A] mt-0.5">
            Manage granular access policies, rate limits, and priority rules enforced at the ALE layer
          </p>
        </div>

        <div className="flex items-center space-x-3">
          <button
            onClick={() => setIsModalOpen(true)}
            className="flex items-center space-x-2 px-3.5 py-1.5 rounded-lg bg-[#91DB79] hover:bg-[#91DB79]/90 text-[#0B0F10] text-xs font-bold transition-colors shadow-sm"
          >
            <Plus className="w-4 h-4" />
            <span>Add Custom Rule</span>
          </button>
        </div>
      </div>

      {/* Rules Table */}
      <div className="rounded-xl bg-[#12191A] border border-[#192223] overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-[#0B0F10]/60 border-b border-[#192223] text-[#91A39A] font-mono">
              <tr>
                <th className="py-3 px-4">Application</th>
                <th className="py-3 px-3">Executable Path</th>
                <th className="py-3 px-3">Policy Type</th>
                <th className="py-3 px-3">Direction</th>
                <th className="py-3 px-3">Limit / Action</th>
                <th className="py-3 px-3">Status</th>
                <th className="py-3 px-4 text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[#192223]/60 font-sans">
              {rules.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-[#91A39A]">
                    No custom rules configured. Click "Add Custom Rule" or block an application from the Applications view.
                  </td>
                </tr>
              ) : (
                rules.map((rule) => (
                  <tr key={rule.id} className="hover:bg-[#192223]/30 transition-colors">
                    <td className="py-3 px-4 font-semibold text-[#EDF5F0]">
                      {rule.app_name}
                    </td>
                    <td className="py-3 px-3 font-mono text-[#91A39A] max-w-xs truncate" title={rule.executable_path}>
                      {truncateMiddle(rule.executable_path, 40)}
                    </td>
                    <td className="py-3 px-3">
                      <span className={`inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[11px] font-mono font-medium ${
                        rule.rule_type === 'block'
                          ? 'bg-[#EF6B70]/15 text-[#EF6B70] border border-[#EF6B70]/30'
                          : rule.rule_type === 'allow'
                          ? 'bg-[#91DB79]/15 text-[#91DB79] border border-[#91DB79]/30'
                          : rule.rule_type === 'priority'
                          ? 'bg-[#57C7B5]/15 text-[#57C7B5] border border-[#57C7B5]/30'
                          : 'bg-[#E9B65C]/15 text-[#E9B65C] border border-[#E9B65C]/30'
                      }`}>
                        {rule.rule_type.toUpperCase()}
                      </span>
                    </td>
                    <td className="py-3 px-3 font-mono uppercase text-[#91A39A]">
                      {rule.direction}
                    </td>
                    <td className="py-3 px-3 font-mono text-[#EDF5F0]">
                      {rule.limit_bytes_per_sec ? formatSpeed(rule.limit_bytes_per_sec) : 'Unrestricted'}
                    </td>
                    <td className="py-3 px-3">
                      <button
                        onClick={() => handleToggleRule(rule)}
                        className="flex items-center space-x-1 text-xs"
                      >
                        {rule.enabled ? (
                          <span className="text-[#91DB79] font-medium flex items-center gap-1">
                            <ToggleRight className="w-5 h-5" /> Enabled
                          </span>
                        ) : (
                          <span className="text-[#91A39A] flex items-center gap-1">
                            <ToggleLeft className="w-5 h-5" /> Disabled
                          </span>
                        )}
                      </button>
                    </td>
                    <td className="py-3 px-4 text-right">
                      <button
                        onClick={() => onDeleteRule(rule.id, rule.executable_path)}
                        className="p-1.5 rounded-lg text-[#91A39A] hover:text-[#EF6B70] hover:bg-[#EF6B70]/10 transition-colors"
                        title="Delete Rule"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Add Custom Rule Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 bg-[#0B0F10]/80 backdrop-blur-sm z-50 flex items-center justify-center p-4">
          <div className="bg-[#12191A] border border-[#192223] rounded-2xl w-full max-w-md p-6 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between pb-3 border-b border-[#192223]">
              <h3 className="text-base font-bold text-[#EDF5F0]">Create Network Rule</h3>
              <button
                onClick={() => setIsModalOpen(false)}
                className="text-[#91A39A] hover:text-[#EDF5F0]"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleCreateRule} className="space-y-4 text-xs">
              <div>
                <label className="block text-[#91A39A] mb-1">Application Name</label>
                <input
                  type="text"
                  value={appName}
                  onChange={(e) => setAppName(e.target.value)}
                  placeholder="e.g., Google Chrome"
                  className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-sm text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
                />
              </div>

              <div>
                <label className="block text-[#91A39A] mb-1">Executable Full Path *</label>
                <input
                  type="text"
                  required
                  value={exePath}
                  onChange={(e) => setExePath(e.target.value)}
                  placeholder="e.g., C:\Program Files\Google\Chrome\Application\chrome.exe"
                  className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-sm text-[#EDF5F0] focus:outline-none focus:border-[#91DB79] font-mono text-[11px]"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-[#91A39A] mb-1">Rule Policy</label>
                  <select
                    value={ruleType}
                    onChange={(e) => setRuleType(e.target.value as RuleType)}
                    className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
                  >
                    <option value="block">Block Internet</option>
                    <option value="allow">Allow Internet</option>
                    <option value="download_limit">Download Limit</option>
                    <option value="upload_limit">Upload Limit</option>
                    <option value="bidirectional_limit">Bidirectional Limit</option>
                    <option value="priority">Priority</option>
                    <option value="exclusive_exemption">Exclusive Exemption</option>
                  </select>
                </div>

                <div>
                  <label className="block text-[#91A39A] mb-1">Direction</label>
                  <select
                    value={direction}
                    onChange={(e) => setDirection(e.target.value as Direction)}
                    className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
                  >
                    <option value="both">Both (In & Out)</option>
                    <option value="outbound">Outbound Only</option>
                    <option value="inbound">Inbound Only</option>
                  </select>
                </div>
              </div>

              {ruleType.includes('limit') && (
                <div>
                  <label className="block text-[#91A39A] mb-1">Speed Limit (KB/s)</label>
                  <input
                    type="number"
                    min="1"
                    value={limitKbps}
                    onChange={(e) => setLimitKbps(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-[#0B0F10] border border-[#192223] text-xs text-[#EDF5F0] focus:outline-none focus:border-[#91DB79]"
                  />
                </div>
              )}

              <div className="pt-2 flex items-center justify-end space-x-3">
                <button
                  type="button"
                  onClick={() => setIsModalOpen(false)}
                  className="px-4 py-2 rounded-lg bg-[#192223] hover:bg-[#212D2E] text-[#EDF5F0]"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="px-4 py-2 rounded-lg bg-[#91DB79] hover:bg-[#91DB79]/90 text-[#0B0F10] font-bold"
                >
                  Save Policy
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
