"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  CreditCard,
  LayoutDashboard,
  LayoutGrid,
  LogOut,
  PieChart,
  Settings,
  Shield,
  Wallet,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { userAgentApi, walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

// /agent and /activity were placeholder Sprint 1 nav items whose routes
// were never built — clicking them 404'd. The dashboard already shows the
// agent's reasoning feed and decision history, so the dedicated routes
// are unnecessary. Keep the surfaces that actually exist.
const BASE_NAV_ITEMS = [
  { href: "/dashboard", icon: LayoutDashboard, label: "Dashboard" },
  { href: "/wallet", icon: Wallet, label: "Wallet" },
  { href: "/portfolio", icon: PieChart, label: "Portfolio" },
  { href: "/strategies", icon: LayoutGrid, label: "Strategies" },
  { href: "/settings", icon: Settings, label: "Settings" },
];

const NAV_ITEMS = PRICING_UI_ENABLED
  ? [
      ...BASE_NAV_ITEMS,
      { href: "/settings/billing", icon: CreditCard, label: "Billing" },
    ]
  : BASE_NAV_ITEMS;

export function Sidebar({ onClose }: { onClose?: () => void }) {
  const pathname = usePathname();
  const router = useRouter();
  const [agentPaused, setAgentPaused] = useState<boolean | null>(null);

  const wallet = usePortfolioStore((s) => s.wallet);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);

  const handleLogout = async () => {
    try {
      await walletApi.logout();
    } catch {
      /* already unauthed */
    }
    setWallet(null);
    setPortfolios([]);
    setUnifiedUsdc(0);
    setUnifiedEurc(0);
    localStorage.removeItem("aegis_email");
    router.push("/login");
  };

  useEffect(() => {
    userAgentApi
      .status()
      .then((s) => setAgentPaused(s.pausedAt !== null))
      .catch(() => {});
  }, []);

  return (
    <aside
      className="w-[220px] h-full shrink-0 flex flex-col border-r border-border-default bg-surface"
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
      <nav className="flex-1 overflow-y-auto px-3 py-4 space-y-0.5">
        {NAV_ITEMS.map(({ href, icon: Icon, label }) => {
          const active = pathname === href || pathname.startsWith(`${href}/`);
          return (
            <Link
              key={href}
              href={href}
              aria-current={active ? "page" : undefined}
              className={cn(
                "flex items-center gap-3 px-3 py-2 rounded-sharp text-xs font-mono transition-all min-h-[44px]",
                active
                  ? "bg-accent-agent/10 text-accent-agent font-medium"
                  : "text-text-lo hover:text-text-hi hover:bg-raised",
              )}
            >
              <Icon className="w-4 h-4 shrink-0" aria-hidden="true" />
              {label}
            </Link>
          );
        })}
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
      {wallet && (
        <div className="px-4 py-3 border-t border-border-default flex items-center gap-2 min-w-0">
          <div className="w-6 h-6 rounded-sharp bg-raised border border-border-default flex items-center justify-center shrink-0">
            <Wallet className="w-3 h-3 text-text-lo" />
          </div>
          <span className="text-[11px] font-mono text-text-mut truncate flex-1">
            {wallet.arcAddress.slice(0, 6)}…{wallet.arcAddress.slice(-4)}
          </span>
          <button
            type="button"
            data-testid="sidebar-logout"
            onClick={() => void handleLogout()}
            title="Log out"
            aria-label="Log out"
            className="shrink-0 p-1 rounded-sharp text-text-mut hover:text-risk hover:bg-risk/5 transition-colors"
          >
            <LogOut className="w-3.5 h-3.5" />
          </button>
        </div>
      )}
    </aside>
  );
}
