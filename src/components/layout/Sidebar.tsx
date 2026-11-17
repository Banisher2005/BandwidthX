import React from 'react';
import { 
  Gauge, 
  Layers, 
  Server, 
  Zap, 
  Settings,
  ShieldCheck,
} from 'lucide-react';
import { PageType } from '../../types';

interface SidebarProps {
  currentPage: PageType;
  onNavigate: (page: PageType) => void;
  exclusiveActive: boolean;
  blockedCount: number;
}

export const Sidebar: React.FC<SidebarProps> = ({
  currentPage,
  onNavigate,
  exclusiveActive,
  blockedCount,
}) => {
  const navItems = [
    { 
      id: 'network' as PageType, 
      label: 'Network', 
      icon: Gauge,
      badge: 'Live',
      badgeColor: 'bg-accent-secondary/15 text-accent-secondary border border-accent-secondary/30',
    },
    { 
      id: 'applications' as PageType, 
      label: 'Applications', 
      icon: Layers,
      badge: blockedCount > 0 ? `${blockedCount} blocked` : undefined,
      badgeColor: 'bg-status-danger/15 text-status-danger border border-status-danger/30',
    },
    { 
      id: 'services' as PageType, 
      label: 'Windows Services', 
      icon: Server,
    },
    { 
      id: 'exclusive' as PageType, 
      label: 'Exclusive Mode', 
      icon: Zap,
      badge: exclusiveActive ? 'ACTIVE' : undefined,
      badgeColor: 'bg-status-danger text-background font-bold animate-pulse',
    },
    { 
      id: 'settings' as PageType, 
      label: 'Settings', 
      icon: Settings,
    },
  ];

  return (
    <aside className="w-64 border-r border-glass-border bg-background/80 backdrop-blur-xl flex flex-col justify-between p-4 select-none shrink-0 z-20">
      <div className="space-y-6">
        {/* App Title Brand */}
        <div className="px-3 pt-2">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-xl bg-gradient-to-br from-accent-primary to-accent-secondary flex items-center justify-center shadow-lg shadow-accent-primary/20">
              <Zap className="w-4 h-4 text-background fill-background" />
            </div>
            <div>
              <div className="text-base font-semibold tracking-tight text-content-primary flex items-center space-x-1.5">
                <span>BandwidthX</span>
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded-full bg-accent-primary/10 text-accent-primary border border-accent-primary/20">3.0</span>
              </div>
              <div className="text-[11px] text-content-secondary font-medium">Windows Network Control</div>
            </div>
          </div>
        </div>

        {/* Navigation list */}
        <nav className="space-y-1.5">
          <div className="text-[10px] font-mono uppercase tracking-wider text-content-muted px-3 py-1 font-semibold">
            Menu
          </div>
          {navItems.map((item) => {
            const Icon = item.icon;
            // Match both new ID and legacy ID for smooth transitions
            const isActive = currentPage === item.id || 
              (item.id === 'network' && (currentPage === 'overview' || currentPage === 'analytics')) ||
              (item.id === 'applications' && currentPage === 'rules');

            return (
              <button
                key={item.id}
                onClick={() => onNavigate(item.id)}
                className={`w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl text-sm font-medium transition-all duration-200 ${
                  isActive
                    ? 'glass-panel-elevated text-accent-primary shadow-lg shadow-black/40 border border-white/10'
                    : 'text-content-secondary hover:text-content-primary hover:bg-white/[0.04]'
                }`}
              >
                <div className="flex items-center space-x-3">
                  <Icon className={`w-4 h-4 transition-colors ${isActive ? 'text-accent-primary' : 'text-content-secondary'}`} />
                  <span className="tracking-tight">{item.label}</span>
                </div>
                {item.badge && (
                  <span className={`text-[10px] font-mono px-2 py-0.5 rounded-full ${item.badgeColor}`}>
                    {item.badge}
                  </span>
                )}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Safety Notice Footer */}
      <div className="rounded-2xl glass-card p-3.5 text-xs">
        <div className="flex items-center space-x-2 text-accent-primary font-medium mb-1">
          <ShieldCheck className="w-4 h-4 text-accent-primary shrink-0" />
          <span className="font-semibold text-[11px] tracking-wide uppercase">WFP Kernel Shield</span>
        </div>
        <p className="text-[11px] leading-relaxed text-content-secondary">
          Dynamic session active. All network policies revert automatically if BandwidthX is closed.
        </p>
      </div>
    </aside>
  );
};
