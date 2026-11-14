import React from 'react';
import { 
  Activity, 
  Layers, 
  Zap, 
  BarChart3, 
  SlidersHorizontal, 
  Settings,
  ShieldAlert
} from 'lucide-react';
import { PageType } from '../../types';

interface SidebarProps {
  currentPage: PageType;
  onNavigate: (page: PageType) => void;
  exclusiveActive: boolean;
  activeRulesCount: number;
}

export const Sidebar: React.FC<SidebarProps> = ({
  currentPage,
  onNavigate,
  exclusiveActive,
  activeRulesCount,
}) => {
  const navItems = [
    { id: 'overview' as PageType, label: 'Overview', icon: Activity },
    { id: 'applications' as PageType, label: 'Applications', icon: Layers },
    { 
      id: 'exclusive' as PageType, 
      label: 'Exclusive Mode', 
      icon: Zap,
      badge: exclusiveActive ? 'ACTIVE' : undefined,
      badgeColor: 'bg-[#EF6B70] text-[#0B0F10]',
    },
    { id: 'analytics' as PageType, label: 'Network Analytics', icon: BarChart3 },
    { 
      id: 'rules' as PageType, 
      label: 'Rules & Controls', 
      icon: SlidersHorizontal,
      badge: activeRulesCount > 0 ? `${activeRulesCount}` : undefined,
      badgeColor: 'bg-[#192223] text-[#57C7B5] border border-[#57C7B5]/30',
    },
    { id: 'settings' as PageType, label: 'Settings', icon: Settings },
  ];

  return (
    <aside className="w-64 border-r border-[#192223] bg-[#0B0F10] flex flex-col justify-between p-4 select-none">
      <nav className="space-y-1">
        <div className="text-[11px] font-mono uppercase tracking-wider text-[#91A39A]/60 px-3 py-2">
          Navigation
        </div>
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = currentPage === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onNavigate(item.id)}
              className={`w-full flex items-center justify-between px-3.5 py-2.5 rounded-lg text-sm font-medium transition-all duration-150 ${
                isActive
                  ? 'bg-[#12191A] text-[#91DB79] border border-[#91DB79]/20 shadow-sm'
                  : 'text-[#91A39A] hover:bg-[#12191A]/60 hover:text-[#EDF5F0]'
              }`}
            >
              <div className="flex items-center space-x-3">
                <Icon className={`w-4 h-4 ${isActive ? 'text-[#91DB79]' : 'text-[#91A39A]'}`} />
                <span>{item.label}</span>
              </div>
              {item.badge && (
                <span className={`text-[10px] font-mono px-1.5 py-0.5 rounded font-bold ${item.badgeColor}`}>
                  {item.badge}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Safety Notice Footer */}
      <div className="rounded-lg bg-[#12191A] border border-[#192223] p-3 text-xs text-[#91A39A]">
        <div className="flex items-center space-x-2 text-[#57C7B5] font-medium mb-1">
          <ShieldAlert className="w-3.5 h-3.5" />
          <span>WFP Dynamic Safety</span>
        </div>
        <p className="text-[11px] leading-relaxed text-[#91A39A]/80">
          Filters operate within a dynamic kernel session. Network access automatically reverts if BandwidthX exits.
        </p>
      </div>
    </aside>
  );
};
