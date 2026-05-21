"use client";

import { useEffect, type ComponentType } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  BarChart3,
  Bot,
  CreditCard,
  CircleHelp,
  LayoutDashboard,
  LayoutGrid,
  ListChecks,
  LogOut,
  PieChart,
  ReceiptText,
  Settings,
  Shield,
  SquareTerminal,
  Wallet,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { userAgentApi, walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

interface NavItem {
  href: string;
  icon: ComponentType<{ className?: string }>;
  label: string;
  match?: string[];
  exact?: boolean;
}

interface NavSection {
  label: string;
  items: NavItem[];
}

const BASE_NAV_SECTIONS: NavSection[] = [
  {
    label: "Portfolio",
    items: [
      { href: "/dashboard", icon: LayoutDashboard, label: "Dashboard" },
      { href: "/wallets", icon: Wallet, label: "Wallets", match: ["/wallet"] },
      { href: "/portfolio", icon: PieChart, label: "Portfolio" },
      { href: "/strategies", icon: LayoutGrid, label: "Strategies" },
      { href: "/transactions", icon: ListChecks, label: "Transactions" },
      { href: "/analytics", icon: BarChart3, label: "Analytics" },
    ],
  },
  {
    label: "Agent",
    items: [
      { href: "/agent-logs", icon: SquareTerminal, label: "Agent Logs" },
      { href: "/agent-studio", icon: Bot, label: "Agent Studio" },
      { href: "/settings/peg", icon: Shield, label: "Peg defense" },
    ],
  },
  {
    label: "Account",
    items: [
      { href: "/tax-center", icon: ReceiptText, label: "Tax center" },
      { href: "/settings", icon: Settings, label: "Settings", exact: true },
      { href: "/help", icon: CircleHelp, label: "Help" },
    ],
  },
];

const NAV_SECTIONS = PRICING_UI_ENABLED
  ? BASE_NAV_SECTIONS.map((section) =>
      section.label === "Account"
        ? {
            ...section,
            items: [
              ...section.items.slice(0, 2),
              { href: "/settings/billing", icon: CreditCard, label: "Billing" },
              ...section.items.slice(2),
            ],
          }
        : section,
    )
  : BASE_NAV_SECTIONS;

function isActivePath(pathname: string, item: NavItem) {
  const paths = [item.href, ...(item.match ?? [])];
  return paths.some((path) =>
    item.exact
      ? pathname === path
      : pathname === path || pathname.startsWith(`${path}/`),
  );
}

export function Sidebar({ onClose }: { onClose?: () => void }) {
  const pathname = usePathname();
  const router = useRouter();

  const wallet = usePortfolioStore((s) => s.wallet);
  const resetSession = usePortfolioStore((s) => s.resetSession);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const agentPausedAt = usePortfolioStore((s) => s.agentPausedAt);
  const setAgentPausedAt = usePortfolioStore((s) => s.setAgentPausedAt);
  const agentPaused = agentPausedAt !== null;

  const handleLogout = async () => {
    try {
      await walletApi.logout();
    } catch {
      /* already unauthed */
    }
    resetSession();
    router.push("/login");
  };

  useEffect(() => {
    userAgentApi
      .status()
      .then((s) => setAgentPausedAt(s.pausedAt))
      .catch(() => {});
  }, [setAgentPausedAt]);

  return (
    <aside
      className="w-[260px] h-full shrink-0 flex flex-col border-r border-border-default bg-surface"
      aria-label="Primary navigation"
    >
      {/* Logo */}
      <div className="flex items-center gap-2.5 px-5 h-16 shrink-0 border-b border-border-default">
        <div
          className="w-7 h-7 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black shrink-0"
          aria-hidden="true"
        >
          <Shield className="w-3.5 h-3.5 text-black" />
        </div>
        <span className="font-bold text-text-hi text-sm tracking-tight font-mono">
          Aegis
        </span>
        <span className="hidden xl:inline text-[10px] font-mono uppercase tracking-widest text-text-mut">
          Console
        </span>
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            aria-label="Close navigation"
            className="ml-auto p-2 rounded-sharp text-text-lo hover:text-text-hi hover:bg-raised transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto px-3 py-3 space-y-4">
        {NAV_SECTIONS.map((section) => (
          <div key={section.label}>
            <p className="px-3 pb-1.5 text-[10px] font-mono uppercase tracking-widest text-text-mut">
              {section.label}
            </p>
            <div className="space-y-0.5">
              {section.items.map((item) => {
                const Icon = item.icon;
                const active = isActivePath(pathname, item);
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    aria-current={active ? "page" : undefined}
                    className={cn(
                      "group flex min-h-9 items-center gap-3 rounded-sharp border px-3 py-2 text-xs font-mono transition-colors",
                      active
                        ? "border-accent-agent/40 bg-accent-agent/10 text-accent-agent"
                        : "border-transparent text-text-lo hover:border-border-default hover:bg-raised hover:text-text-hi",
                    )}
                  >
                    <Icon
                      className={cn(
                        "h-4 w-4 shrink-0",
                        active
                          ? "text-accent-agent"
                          : "text-text-mut group-hover:text-text-hi",
                      )}
                      aria-hidden="true"
                    />
                    <span className="truncate">{item.label}</span>
                  </Link>
                );
              })}
            </div>
          </div>
        ))}
      </nav>

      {/* Agent status indicator */}
      <div className="px-4 py-4 border-t border-border-default">
        {agentPaused ? (
          <div className="flex items-center gap-2 px-3 py-2 rounded-sharp bg-warn/5 border border-warn/30">
            <span className="w-1.5 h-1.5 rounded-sharp bg-warn shrink-0" />
            <span className="text-xs text-warn font-mono uppercase tracking-widest">
              Agent paused
            </span>
          </div>
        ) : (
          <div className="flex items-center gap-2 px-3 py-2 rounded-sharp bg-accent-agent/5 border border-accent-agent/30">
            <span className="w-1.5 h-1.5 rounded-sharp bg-accent-agent animate-pulse shrink-0" />
            <span className="text-xs text-accent-agent font-mono uppercase tracking-widest">
              Agent active
            </span>
          </div>
        )}
      </div>

      {/* Account row */}
      {(wallet || sessionActive) && (
        <div className="px-4 py-3 border-t border-border-default space-y-2">
          <div className="flex items-center gap-2 min-w-0">
            <div className="w-6 h-6 rounded-sharp bg-raised border border-border-default flex items-center justify-center shrink-0">
              <Wallet className="w-3 h-3 text-text-lo" />
            </div>
            <span className="text-[11px] font-mono text-text-mut truncate flex-1">
              {wallet
                ? `${wallet.arcAddress.slice(0, 6)}…${wallet.arcAddress.slice(-4)}`
                : "Session active"}
            </span>
          </div>
          <button
            type="button"
            data-testid="sidebar-logout"
            onClick={() => void handleLogout()}
            title="Log out"
            aria-label="Log out"
            className="w-full min-h-[36px] inline-flex items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-3 text-xs font-mono text-text-lo hover:border-risk/50 hover:bg-risk/5 hover:text-risk transition-colors"
          >
            <LogOut className="w-3.5 h-3.5" aria-hidden="true" />
            Log out
          </button>
        </div>
      )}
    </aside>
  );
}
