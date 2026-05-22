"use client";

import { useEffect, useState, type ComponentType } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  BarChart3,
  Bot,
  CreditCard,
  CircleHelp,
  Compass,
  LayoutDashboard,
  LayoutGrid,
  ListChecks,
  LockKeyhole,
  LogOut,
  PieChart,
  ReceiptText,
  Settings,
  Shield,
  SquareTerminal,
  Trophy,
  Wallet,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { userAgentApi, walletApi } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";
import { usePortfolioStore } from "@/stores/portfolio";

interface NavItem {
  href: string;
  icon: ComponentType<{ className?: string }>;
  label: string;
  description: string;
  match?: string[];
  exact?: boolean;
}

interface NavSection {
  label: string;
  description: string;
  tone: "pnl" | "agent" | "neutral";
  items: NavItem[];
}

const BASE_NAV_SECTIONS: NavSection[] = [
  {
    label: "Portfolio",
    description: "Money, targets, approvals",
    tone: "pnl",
    items: [
      {
        href: "/dashboard",
        icon: LayoutDashboard,
        label: "Dashboard",
        description: "cash, targets, review",
      },
      {
        href: "/wallets",
        icon: Wallet,
        label: "Wallet",
        description: "One account, all networks",
        match: ["/wallet"],
      },
      {
        href: "/portfolio",
        icon: PieChart,
        label: "Portfolio",
        description: "positions and target mix",
      },
      {
        href: "/strategies",
        icon: LayoutGrid,
        label: "Strategies",
        description: "adoptable templates",
      },
      {
        href: "/transactions",
        icon: ListChecks,
        label: "Transactions",
        description: "plans and execution",
      },
      {
        href: "/analytics",
        icon: BarChart3,
        label: "Analytics",
        description: "performance diagnostics",
      },
    ],
  },
  {
    label: "Agent",
    description: "AI reasoning and controls",
    tone: "agent",
    items: [
      {
        href: "/agent-logs",
        icon: SquareTerminal,
        label: "Agent Logs",
        description: "decision history",
      },
      {
        href: "/agent-studio",
        icon: Bot,
        label: "Agent Studio",
        description: "ask for advice",
      },
      {
        href: "/settings/peg",
        icon: Shield,
        label: "Peg defense",
        description: "stablecoin triggers",
      },
    ],
  },
  {
    label: "Account",
    description: "Exports and settings",
    tone: "neutral",
    items: [
      {
        href: "/tax-center",
        icon: ReceiptText,
        label: "Tax center",
        description: "CSV and accountant links",
        match: ["/settings/tax"],
      },
      {
        href: "/settings",
        icon: Settings,
        label: "Settings",
        description: "rules and preferences",
        exact: true,
      },
      {
        href: "/help",
        icon: CircleHelp,
        label: "Help",
        description: "plain-English answers",
      },
    ],
  },
  {
    label: "Discover",
    description: "Public product surfaces",
    tone: "agent",
    items: [
      {
        href: "/explore",
        icon: Compass,
        label: "Explore demos",
        description: "read-only examples",
      },
      {
        href: "/leaderboard",
        icon: Trophy,
        label: "Leaderboard",
        description: "public trustability",
      },
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
              {
                href: "/settings/billing",
                icon: CreditCard,
                label: "Billing",
                description: "tiers and invoices",
              },
              ...section.items.slice(2),
            ],
          }
        : section,
    )
  : BASE_NAV_SECTIONS;

const PUBLIC_NAV_HREFS = new Set([
  "/explore",
  "/leaderboard",
  "/strategies",
  "/help",
]);

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

  const wallet = usePortfolioStore((s) => s.wallet);
  const resetSession = usePortfolioStore((s) => s.resetSession);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);
  const agentPausedAt = usePortfolioStore((s) => s.agentPausedAt);
  const setAgentPausedAt = usePortfolioStore((s) => s.setAgentPausedAt);
  const agentPaused = agentPausedAt !== null;
  const walletPending = sessionActive && !wallet;
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const navSections = NAV_SECTIONS;
  const showLogoutInSidebar = Boolean(onClose);

  const handleLogout = async () => {
    setLogoutError(null);
    try {
      await walletApi.logout();
    } catch (e) {
      setLogoutError(logoutFailureMessage(e));
      return;
    }
    resetSession();
    window.location.replace(logoutRedirect());
  };

  useEffect(() => {
    if (!wallet) {
      setAgentPausedAt(null);
      return;
    }
    userAgentApi
      .status()
      .then((s) => setAgentPausedAt(s.pausedAt))
      .catch(() => {});
  }, [setAgentPausedAt, wallet]);

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
      <nav className="flex-1 overflow-y-auto px-3 py-3 space-y-3">
        {navSections.map((section) => (
          <div key={section.label}>
            <div className="px-3 pb-1.5">
              <div className="flex items-center justify-between gap-2">
                <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
                  {section.label}
                </p>
                <span
                  aria-hidden="true"
                  className={cn(
                    "h-1.5 w-8 border border-black",
                    section.tone === "pnl"
                      ? "bg-accent-pnl"
                      : section.tone === "agent"
                        ? "bg-accent-agent"
                        : "bg-text-lo",
                  )}
                />
              </div>
              <p className="mt-0.5 truncate text-[10px] font-mono text-text-mut">
                {section.description}
              </p>
            </div>
            <div className="space-y-0.5">
              {section.items.map((item) => {
                const Icon = item.icon;
                const active = isActivePath(pathname, item);
                const publicNav = isPublicNavItem(item);
                const recoveryNav = isWalletRecoveryNavItem(item);
                const locked =
                  sessionResolved &&
                  !publicNav &&
                  (!sessionActive || (!wallet && !recoveryNav));
                const href = locked
                  ? walletPending
                    ? "/wallets"
                    : authHref("/login", item.href)
                  : item.href;
                return (
                  <Link
                    key={item.href}
                    href={href}
                    aria-current={active ? "page" : undefined}
                    title={
                      locked
                        ? walletPending
                          ? `${item.label} will open after account setup finishes`
                          : `${item.label} requires sign in`
                        : item.label
                    }
                    className={cn(
                      "group relative grid min-h-[46px] grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-2 rounded-sharp border px-2.5 py-2 font-mono transition-colors",
                      active
                        ? activeNavClasses(section.tone)
                        : "border-transparent text-text-lo hover:border-border-default hover:bg-raised hover:text-text-hi",
                    )}
                  >
                    {active && (
                      <span
                        aria-hidden="true"
                        className={cn(
                          "absolute left-0 top-1/2 h-7 w-1 -translate-y-1/2 border-y border-r border-black",
                          section.tone === "pnl"
                            ? "bg-accent-pnl"
                            : section.tone === "agent"
                              ? "bg-accent-agent"
                              : "bg-text-hi",
                        )}
                      />
                    )}
                    <Icon
                      className={cn(
                        "h-4 w-4 justify-self-center",
                        active
                          ? iconActiveClass(section.tone)
                          : "text-text-mut group-hover:text-text-hi",
                      )}
                      aria-hidden="true"
                    />
                    <span className="min-w-0">
                      <span className="block truncate text-xs font-semibold">
                        {item.label}
                      </span>
                      <span className="mt-0.5 block truncate text-[10px] text-text-mut">
                        {locked
                          ? lockedDescription(walletPending)
                          : item.description}
                      </span>
                    </span>
                    {locked && (
                      <LockKeyhole
                        className={cn(
                          "h-3.5 w-3.5 shrink-0",
                          "text-text-mut group-hover:text-accent-agent",
                        )}
                        aria-hidden="true"
                      />
                    )}
                    {!locked && active && (
                      <span
                        aria-hidden="true"
                        className={cn(
                          "h-1.5 w-1.5 shrink-0",
                          section.tone === "pnl"
                            ? "bg-accent-pnl"
                            : section.tone === "agent"
                              ? "bg-accent-agent"
                              : "bg-text-hi",
                        )}
                      />
                    )}
                  </Link>
                );
              })}
            </div>
          </div>
        ))}
      </nav>

      {/* Agent status indicator */}
      <div className="px-4 py-4 border-t border-border-default">
        {!sessionResolved ? (
          <div className="flex items-center gap-2 rounded-sharp border border-border-default bg-bg px-3 py-2">
            <span className="font-mono text-xs uppercase tracking-widest text-text-mut">
              Checking session…
            </span>
          </div>
        ) : !sessionActive ? (
          <div className="space-y-3">
            <div className="flex items-center gap-2 rounded-sharp border border-border-default bg-bg px-3 py-2">
              <span className="h-1.5 w-1.5 shrink-0 rounded-sharp bg-text-mut" />
              <span className="font-mono text-xs uppercase tracking-widest text-text-mut">
                Signed out
              </span>
            </div>
            <p className="px-1 text-[11px] font-mono leading-relaxed text-text-mut">
              Browse strategies and help without an account. Continue with email
              to manage balances, approvals, agent runs, and tax exports.
            </p>
            <div className="grid gap-2">
              <Link
                href={authHref("/login", pathname)}
                className="inline-flex min-h-[36px] items-center justify-center rounded-sharp border border-black bg-accent-agent px-2 text-center text-[11px] font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
              >
                Continue
              </Link>
            </div>
          </div>
        ) : walletPending ? (
          <div className="flex items-center gap-2 px-3 py-2 rounded-sharp bg-warn/5 border border-warn/30">
            <span className="w-1.5 h-1.5 rounded-sharp bg-warn shrink-0" />
            <span className="text-xs text-warn font-mono uppercase tracking-widest">
              Account setup pending
            </span>
          </div>
        ) : agentPaused ? (
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
                : "Account setup pending"}
            </span>
          </div>
          {showLogoutInSidebar ? (
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
          ) : (
            <p className="text-[10px] font-mono leading-relaxed text-text-mut">
              Sign out from the top bar. This rail keeps navigation focused.
            </p>
          )}
          {showLogoutInSidebar && logoutError && (
            <p role="alert" className="text-[11px] font-mono text-risk">
              {logoutError}
            </p>
          )}
        </div>
      )}
    </aside>
  );
}

function logoutRedirect() {
  const params = new URLSearchParams({ signedOut: "1" });
  return `/login?${params.toString()}`;
}

function logoutFailureMessage(error: unknown) {
  const message = (error as Error).message.toLowerCase();
  if (message.includes("still accepts")) {
    return "Sign out did not finish. Try again.";
  }
  if (message.includes("verification failed")) {
    return "Aegis could not confirm sign out. Try again.";
  }
  return "Aegis could not sign out. Check the connection and try again.";
}

function authHref(path: "/login", next: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

function isPublicNavItem(item: NavItem) {
  return PUBLIC_NAV_HREFS.has(item.href);
}

function isWalletRecoveryNavItem(item: NavItem) {
  return item.href === "/wallets" || item.href === "/settings";
}

function activeNavClasses(tone: NavSection["tone"]) {
  if (tone === "pnl") {
    return "border-accent-pnl/40 bg-accent-pnl/10 text-accent-pnl";
  }
  if (tone === "agent") {
    return "border-accent-agent/40 bg-accent-agent/10 text-accent-agent";
  }
  return "border-border-hi/40 bg-white/5 text-text-hi";
}

function iconActiveClass(tone: NavSection["tone"]) {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  return "text-text-hi";
}

function lockedDescription(walletPending: boolean) {
  return walletPending ? "account setup pending" : "sign in required";
}
