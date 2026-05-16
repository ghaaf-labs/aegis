"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  CreditCard,
  LayoutDashboard,
  PieChart,
  Settings,
  Shield,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { PRICING_UI_ENABLED } from "@/lib/flags";

// /agent and /activity were placeholder Sprint 1 nav items whose routes
// were never built — clicking them 404'd. The dashboard already shows the
// agent's reasoning feed and decision history, so the dedicated routes
// are unnecessary. Keep the surfaces that actually exist.
const BASE_NAV_ITEMS = [
  { href: "/dashboard", icon: LayoutDashboard, label: "Dashboard" },
  { href: "/portfolio", icon: PieChart, label: "Portfolio" },
  { href: "/settings", icon: Settings, label: "Settings" },
];

const NAV_ITEMS = PRICING_UI_ENABLED
  ? [
      ...BASE_NAV_ITEMS,
      { href: "/settings/billing", icon: CreditCard, label: "Billing" },
    ]
  : BASE_NAV_ITEMS;

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-[220px] shrink-0 flex flex-col border-r border-white/5 bg-gray-950/50">
      {/* Logo */}
      <div className="flex items-center gap-2.5 px-5 py-5 border-b border-white/5">
        <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center shrink-0">
          <Shield className="w-3.5 h-3.5 text-white" />
        </div>
        <span className="font-bold text-white text-sm tracking-tight">
          Aegis
        </span>
        <span className="ml-auto text-[10px] px-1.5 py-0.5 rounded-full bg-blue-500/20 text-blue-400 font-medium">
          AI
        </span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 py-4 space-y-0.5">
        {NAV_ITEMS.map(({ href, icon: Icon, label }) => {
          const active = pathname === href || pathname.startsWith(`${href}/`);
          return (
            <Link
              key={href}
              href={href}
              className={cn(
                "flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-all",
                active
                  ? "bg-blue-600/15 text-blue-400 font-medium"
                  : "text-gray-500 hover:text-gray-300 hover:bg-white/5",
              )}
            >
              <Icon className="w-4 h-4 shrink-0" />
              {label}
            </Link>
          );
        })}
      </nav>

      {/* Agent status indicator */}
      <div className="px-4 py-4 border-t border-white/5">
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-emerald-500/5 border border-emerald-500/15">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
          <span className="text-xs text-emerald-400 font-medium">
            Agent active
          </span>
        </div>
      </div>
    </aside>
  );
}
