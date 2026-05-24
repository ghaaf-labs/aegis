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
  Info,
  LayoutDashboard,
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
import { logoutFailureMessage, logoutRedirect } from "./logout-copy";

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
    description: "Money and targets",
    tone: "pnl",
    items: [
      {
        href: "/dashboard",
        icon: LayoutDashboard,
        label: "Dashboard",
        description: "overview and next step",
      },
      {
        href: "/wallets",
        icon: Wallet,
        label: "Wallets",
        description: "cash and addresses",
        match: ["/wallet"],
      },
      {
        href: "/portfolio",
        icon: PieChart,
        label: "Portfolio",
        description: "positions and targets",
      },
      {
        href: "/transactions",
        icon: ListChecks,
        label: "Transactions",
        description: "reviews and moves",
      },
      {
        href: "/analytics",
        icon: BarChart3,
        label: "Analytics",
        description: "value and decisions",
      },
    ],
  },
  {
    label: "Agent",
    description: "Decisions and controls",
    tone: "agent",
    items: [
      {
        href: "/agent-logs",
        icon: SquareTerminal,
        label: "Agent Logs",
        description: "past decisions",
      },
      {
        href: "/agent-studio",
        icon: Bot,
        label: "Agent Studio",
        description: "ask a question",
      },
      {
        href: "/settings/peg",
        icon: Shield,
        label: "Peg defense",
        description: "stablecoin safety",
      },
    ],
  },
  {
    label: "Account",
    description: "Settings and reports",
    tone: "neutral",
    items: [
      {
        href: "/tax-center",
        icon: ReceiptText,
        label: "Tax center",
        description: "tax reports",
        match: ["/settings/tax"],
      },
      {
        href: "/settings",
        icon: Settings,
        label: "Settings",
        description: "preferences",
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
    description: "Public pages",
    tone: "agent",
    items: [
      {
        href: "/explore",
        icon: Compass,
        label: "Explore demos",
        description: "example portfolios",
      },
      {
        href: "/leaderboard",
        icon: Trophy,
        label: "Leaderboard",
        description: "public results",
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
                description: "plan and invoices",
              },
              ...section.items.slice(2),
            ],
          }
        : section,
    )
  : BASE_NAV_SECTIONS;

const PUBLIC_NAV_HREFS = new Set(["/explore", "/leaderboard", "/help"]);

const SIGNED_OUT_NAV: NavItem[] = [
  {
    href: "/explore",
    icon: Compass,
    label: "Explore demos",
    description: "example portfolios",
  },
  {
    href: "/leaderboard",
    icon: Trophy,
    label: "Leaderboard",
    description: "public results",
  },
  {
    href: "/about",
    icon: Info,
    label: "About",
    description: "how Aegis works",
  },
  {
    href: "/help",
    icon: CircleHelp,
    label: "Help",
    description: "plain-English answers",
  },
];

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
  // Once session is confirmed absent, hide the full protected rail from the
  // DOM entirely — signed-out users only see public destinations.
  const signedOut = sessionResolved && !sessionActive;
  const navSections = NAV_SECTIONS;

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
            className="touch-target ml-auto p-2 rounded-sharp text-text-lo hover:text-text-hi hover:bg-raised transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto px-3 py-3 space-y-3">
        {signedOut ? (
          // Signed-out: render only public destinations — protected links must
          // not appear in the DOM or accessibility tree at all.
          <div>
            <div className="px-3 pb-1.5">
              <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
                Public
              </p>
            </div>
            <div className="space-y-0.5">
              {SIGNED_OUT_NAV.map((item) => {
                const Icon = item.icon;
                const active = isActivePath(pathname, item);
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    aria-label={`${item.label}: ${item.description}`}
                    aria-current={active ? "page" : undefined}
                    title={item.label}
                    className={cn(
                      "group relative grid min-h-[46px] grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-2 rounded-sharp border px-2.5 py-2 font-mono transition-colors",
                      active
                        ? activeNavClasses("agent")
                        : "border-transparent text-text-lo hover:border-border-default hover:bg-raised hover:text-text-hi",
                    )}
                  >
                    {active && (
                      <span
                        aria-hidden="true"
                        className="absolute left-0 top-1/2 h-7 w-1 -translate-y-1/2 border-y border-r border-black bg-accent-agent"
                      />
                    )}
                    <Icon
                      className={cn(
                        "h-4 w-4 justify-self-center",
                        active
                          ? "text-accent-agent"
                          : "text-text-mut group-hover:text-text-hi",
                      )}
                      aria-hidden="true"
                    />
                    <span className="min-w-0">
                      <span className="block truncate text-xs font-semibold">
                        {item.label}
                      </span>
                      <span className="mt-0.5 block truncate text-[10px] text-text-mut">
                        {" "}
                        {item.description}
                      </span>
                    </span>
                    {active && (
                      <span
                        aria-hidden="true"
                        className="h-1.5 w-1.5 shrink-0 bg-accent-agent"
                      />
                    )}
                  </Link>
                );
              })}
            </div>
          </div>
        ) : (
          navSections.map((section) => (
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
                  const itemDescription = locked
                    ? lockedDescription(walletPending)
                    : item.description;
                  return (
                    <Link
                      key={item.href}
                      href={href}
                      aria-label={`${item.label}: ${itemDescription}`}
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
                          {" "}
                          {itemDescription}
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
          ))
        )}
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
              Sign in to manage your account, reviews, and reports.
            </p>
            <div className="grid gap-2">
              <Link
                href={authHref("/login", pathname)}
                className="touch-target inline-flex min-h-[36px] items-center justify-center rounded-sharp border border-black bg-accent-agent px-2 text-center text-[11px] font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
              >
                Sign in
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
          <button
            type="button"
            data-testid="sidebar-logout"
            onClick={() => void handleLogout()}
            title="Sign out"
            aria-label="Sign out"
            className="touch-target inline-flex min-h-11 w-full items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-3 text-xs font-mono text-text-lo transition-colors hover:border-risk/50 hover:bg-risk/5 hover:text-risk"
          >
            <LogOut className="w-3.5 h-3.5" aria-hidden="true" />
            Sign out
          </button>
          {logoutError && (
            <p role="alert" className="text-[11px] font-mono text-risk">
              {logoutError}
            </p>
          )}
        </div>
      )}
    </aside>
  );
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
