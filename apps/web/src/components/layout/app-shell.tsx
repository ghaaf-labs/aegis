"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { CircleAlert, Menu, Wallet, X } from "lucide-react";
import { Sidebar } from "@/components/layout/sidebar";
import { Header } from "@/components/layout/header";
import { ErrorBoundary } from "@/components/error-boundary";
import { safeNextPath } from "@/lib/auth-routing";
import { usePortfolioStore } from "@/stores/portfolio";

export function AppShell({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  const pathname = usePathname();
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const wallet = usePortfolioStore((s) => s.wallet);
  const walletPending = sessionActive && !wallet;
  const mobileAction = walletPending
    ? {
        href: "/onboarding",
        label: "Finish account",
        tone: "warn" as const,
      }
    : sessionActive
      ? null
      : {
          href: authHref("/login", pathname),
          label: "Continue",
          tone: "agent" as const,
        };

  // Close the drawer on every route change so a mobile user doesn't have to
  // dismiss it manually after each tap.
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  return (
    <div className="flex h-screen bg-bg text-text-default overflow-hidden">
      {/* Desktop sidebar — hidden on small screens. */}
      <div className="hidden md:flex">
        <Sidebar />
      </div>

      {/* Mobile drawer — fixed overlay, slides in from the left. */}
      {open && (
        <button
          type="button"
          aria-label="Close navigation"
          className="md:hidden fixed inset-0 z-40 bg-black/60"
          onClick={() => setOpen(false)}
        />
      )}
      {open && (
        <div className="md:hidden fixed inset-y-0 left-0 z-50 w-[min(340px,100vw)]">
          <Sidebar onClose={() => setOpen(false)} />
        </div>
      )}

      <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
        <div className="flex min-h-[58px] items-center gap-2 border-b border-border-default bg-surface px-3 py-2 md:hidden">
          <button
            type="button"
            onClick={() => setOpen((v) => !v)}
            aria-label={open ? "Close navigation" : "Open navigation"}
            aria-expanded={open}
            className="inline-flex min-h-[42px] min-w-[42px] items-center justify-center rounded-sharp border-brutal border-border-default bg-raised"
          >
            {open ? <X className="w-4 h-4" /> : <Menu className="w-4 h-4" />}
          </button>
          <div className="min-w-0 flex-1">
            <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
              Aegis Console
            </p>
            <p className="truncate font-mono text-sm font-semibold text-text-hi">
              {mobileTitle(pathname)}
            </p>
          </div>
          {mobileAction && (
            <Link
              href={mobileAction.href}
              className={
                mobileAction.tone === "warn"
                  ? "inline-flex min-h-[36px] shrink-0 items-center gap-1.5 rounded-sharp border border-warn/40 bg-warn/5 px-2.5 font-mono text-[10px] uppercase tracking-widest text-warn"
                  : "inline-flex min-h-[36px] shrink-0 items-center gap-1.5 rounded-sharp border border-black bg-accent-agent px-2.5 font-mono text-[10px] font-semibold uppercase tracking-widest text-black shadow-brutal-sm"
              }
            >
              {mobileAction.tone === "warn" ? (
                <Wallet className="h-3.5 w-3.5" />
              ) : (
                <CircleAlert className="h-3.5 w-3.5" />
              )}
              {mobileAction.label}
            </Link>
          )}
        </div>
        <div className="hidden md:block">
          <Header />
        </div>
        <main className="flex-1 overflow-y-auto p-4 md:p-6 scrollbar-thin">
          <ErrorBoundary>{children}</ErrorBoundary>
        </main>
      </div>
    </div>
  );
}

function mobileTitle(pathname: string) {
  const first = pathname.split("/").filter(Boolean)[0] ?? "dashboard";
  const labels: Record<string, string> = {
    "agent-logs": "Agent Logs",
    "agent-studio": "Agent Studio",
    analytics: "Analytics",
    dashboard: "Dashboard",
    explore: "Explore Demos",
    help: "Help",
    leaderboard: "Leaderboard",
    portfolio: "Portfolio",
    rebalance: "Rebalance Review",
    settings: "Settings",
    strategies: "Strategies",
    transactions: "Transactions",
    wallet: "Wallet",
    wallets: "Wallets",
    "tax-center": "Tax Center",
  };
  return labels[first] ?? titleCase(first);
}

function titleCase(value: string) {
  return value
    .split("-")
    .filter(Boolean)
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

function authHref(path: "/login", next: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}
