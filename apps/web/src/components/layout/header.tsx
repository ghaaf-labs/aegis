"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  Bell,
  Check,
  ChevronDown,
  Layers,
  LogIn,
  LogOut,
  Plus,
  Wifi,
  WifiOff,
  Wallet as WalletIcon,
} from "lucide-react";
import { BrutalButton, BrutalPill } from "@aegis/ui";
import type { Portfolio } from "@/types";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { gatewayApi, walletApi } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";
import { logoutFailureMessage, logoutRedirect } from "./logout-copy";

export function Header() {
  const router = useRouter();
  const pathname = usePathname();
  const portfolio = useActivePortfolio();
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const setActivePortfolio = usePortfolioStore((s) => s.setActivePortfolio);
  const sseConnected = usePortfolioStore((s) => s.sseConnected);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const setGatewayBalanceStatus = usePortfolioStore(
    (s) => s.setGatewayBalanceStatus,
  );
  const wallet = usePortfolioStore((s) => s.wallet);
  const resetSession = usePortfolioStore((s) => s.resetSession);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const walletPending = sessionActive && !wallet;

  const [notifOpen, setNotifOpen] = useState(false);
  const [userOpen, setUserOpen] = useState(false);
  const [portfolioOpen, setPortfolioOpen] = useState(false);
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const portfolioRef = useRef<HTMLDivElement>(null);
  const notifRef = useRef<HTMLDivElement>(null);
  const userRef = useRef<HTMLDivElement>(null);
  const pegAlerts = usePortfolioStore((s) => s.pegAlerts);

  const handleLogout = async () => {
    setLogoutError(null);
    try {
      await walletApi.logout();
    } catch (e) {
      setLogoutError(logoutFailureMessage(e));
      return;
    }
    resetSession();
    setUserOpen(false);
    window.location.replace(logoutRedirect());
  };

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        portfolioRef.current &&
        !portfolioRef.current.contains(e.target as Node)
      )
        setPortfolioOpen(false);
      if (notifRef.current && !notifRef.current.contains(e.target as Node))
        setNotifOpen(false);
      if (userRef.current && !userRef.current.contains(e.target as Node))
        setUserOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const switchPortfolio = (id: string) => {
    setActivePortfolio(id);
    setPortfolioOpen(false);
    router.push(`/dashboard/${id}`);
  };
  const activePortfolioName = portfolio ? displayPortfolioName(portfolio) : "";

  // Fetch unified balance on first render — Gateway SSE keeps it fresh after.
  useEffect(() => {
    if (!wallet) return;
    let alive = true;
    setGatewayBalanceStatus("loading");
    void gatewayApi
      .balance()
      .then((b) => {
        if (!alive) return;
        setUnifiedUsdc(b.unifiedUsdc);
        setUnifiedEurc(b.unifiedEurc);
        setPerChain(b.perChain ?? {}, b.perChainEurc ?? {});
        setGatewayBalanceStatus("ready");
      })
      .catch((err) => {
        // Best-effort hydration; SSE will overwrite this once the channel
        // opens. Surface in dev so we can spot persistent gateway outages.
        if (alive) {
          setGatewayBalanceStatus("error", "Wallet balance is unavailable.");
          console.warn("gateway balance hydrate failed", err);
        }
      });
    return () => {
      alive = false;
    };
  }, [
    wallet,
    setUnifiedUsdc,
    setUnifiedEurc,
    setPerChain,
    setGatewayBalanceStatus,
  ]);

  return (
    <header className="flex items-center justify-between px-6 h-16 border-b-brutal border-border-default bg-surface shrink-0">
      <div className="flex items-center gap-4">
        {portfolio && portfolios.length > 1 ? (
          <div ref={portfolioRef} className="relative">
            <button
              type="button"
              onClick={() => setPortfolioOpen((v) => !v)}
              aria-expanded={portfolioOpen}
              aria-haspopup="menu"
              className="flex min-h-[38px] max-w-[360px] items-center gap-2 border-brutal border-border-default bg-bg px-3 py-1.5 text-left hover:border-border-hi"
            >
              <Layers className="h-4 w-4 shrink-0 text-accent-agent" />
              <span className="min-w-0">
                <span className="block text-[10px] font-mono uppercase tracking-widest text-text-mut">
                  Portfolio
                </span>
                <span className="block truncate text-sm font-semibold text-text-hi">
                  {activePortfolioName}
                </span>
              </span>
              <span className="ml-1 shrink-0 border border-border-default px-1.5 py-0.5 text-[10px] font-mono text-text-lo">
                {portfolios.length}
              </span>
              <ChevronDown className="h-3.5 w-3.5 shrink-0 text-text-mut" />
            </button>
            {portfolioOpen && (
              <div
                role="menu"
                aria-label="Switch portfolio"
                className="absolute left-0 top-full z-50 mt-2 w-[360px] border-brutal border-border-default bg-surface shadow-brutal"
              >
                <div className="border-b border-border-default px-3 py-2">
                  <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
                    Switch portfolio
                  </p>
                  <p className="mt-1 text-[11px] font-mono text-text-lo">
                    Each portfolio has its own target, decisions, and approval
                    history. Wallet cash stays shared.
                  </p>
                </div>
                <div className="max-h-72 overflow-y-auto">
                  {portfolios.map((p) => {
                    const active = p.id === portfolio.id;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        role="menuitem"
                        onClick={() => switchPortfolio(p.id)}
                        className="grid w-full grid-cols-[20px_1fr_auto] items-center gap-2 border-b border-border-default px-3 py-2 text-left last:border-b-0 hover:bg-raised"
                      >
                        <span className="flex h-5 w-5 items-center justify-center">
                          {active ? (
                            <Check className="h-3.5 w-3.5 text-accent-agent" />
                          ) : (
                            <span className="h-1.5 w-1.5 bg-border-hi" />
                          )}
                        </span>
                        <span className="min-w-0">
                          <span className="block truncate text-xs font-mono font-semibold text-text-hi">
                            {displayPortfolioName(p)}
                          </span>
                          <span className="block truncate text-[10px] font-mono text-text-mut">
                            {portfolioSubtitle(p)}
                          </span>
                        </span>
                        <span className="text-right text-[10px] font-mono text-text-lo tabular-nums">
                          {formatCompactUsd(p.totalValueUsd)}
                        </span>
                      </button>
                    );
                  })}
                </div>
                <div className="grid grid-cols-2 border-t border-border-default">
                  <Link
                    href="/strategies"
                    onClick={() => setPortfolioOpen(false)}
                    className="inline-flex items-center justify-center gap-1 border-r border-border-default px-3 py-2 text-[11px] font-mono text-accent-agent hover:bg-raised"
                  >
                    <Layers className="h-3 w-3" />
                    Adopt strategy
                  </Link>
                  <Link
                    href="/onboarding"
                    onClick={() => setPortfolioOpen(false)}
                    className="inline-flex items-center justify-center gap-1 px-3 py-2 text-[11px] font-mono text-accent-pnl hover:bg-raised"
                  >
                    <Plus className="h-3 w-3" />
                    Custom target
                  </Link>
                </div>
              </div>
            )}
          </div>
        ) : portfolio ? (
          <div className="flex items-center gap-2 px-3 py-1.5 border-brutal border-border-default rounded-sharp bg-bg">
            <span className="text-xs text-text-lo font-mono">Portfolio</span>
            <span className="text-sm font-semibold text-text-hi">
              {activePortfolioName}
            </span>
          </div>
        ) : null}

        {/* PORTFOLIO VALUE / ALL-TIME PNL / GATEWAY all duplicated the
            Net Worth card on the dashboard, with one critical bug — the
            header showed `portfolio.totalValueUsd` ("invested only") while
            the card showed invested + wallet. Same label, different number.
            Single source of truth lives on the Net Worth card now. */}
      </div>

      <div className="flex items-center gap-3 ml-auto">
        {wallet ? (
          <BrutalPill tone={sseConnected ? "pnl" : "neutral"}>
            {sseConnected ? (
              <Wifi className="w-3 h-3" />
            ) : (
              <WifiOff className="w-3 h-3" />
            )}
            <span>{sseConnected ? "STREAM" : "OFFLINE"}</span>
          </BrutalPill>
        ) : walletPending ? (
          <Link
            href="/wallets"
            className="inline-flex min-h-[36px] items-center justify-center gap-2 rounded-sharp border border-warn/40 bg-warn/5 px-3 text-[10px] font-mono uppercase tracking-widest text-warn transition-colors hover:bg-warn/10"
          >
            <WalletIcon className="h-3.5 w-3.5" />
            Account setup
          </Link>
        ) : null}
        {sessionActive ? (
          <div ref={notifRef} className="relative">
            <BrutalButton
              variant="ghost"
              className="text-text-lo relative"
              aria-label="Notifications"
              onClick={() => setNotifOpen((v) => !v)}
            >
              <Bell className="w-4 h-4" />
              {pegAlerts.length > 0 && (
                <span className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-risk" />
              )}
            </BrutalButton>
            {notifOpen && (
              <div className="absolute right-0 top-full mt-2 w-72 border-brutal border-border-default bg-surface shadow-brutal z-50 rounded-sharp">
                <div className="px-3 py-2 border-b border-border-default flex items-center justify-between">
                  <span className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
                    Notifications
                  </span>
                  {pegAlerts.length > 0 && (
                    <span className="text-[10px] font-mono text-risk">
                      {pegAlerts.length} peg alert
                      {pegAlerts.length !== 1 ? "s" : ""}
                    </span>
                  )}
                </div>
                {pegAlerts.length === 0 ? (
                  <div className="px-3 py-4 text-xs font-mono text-text-mut text-center">
                    No notifications
                  </div>
                ) : (
                  <ul className="max-h-56 overflow-y-auto">
                    {pegAlerts.slice(0, 8).map((a, i) => (
                      <li
                        key={`${a.ruleId}-${a.observedAt}-${i}`}
                        className="px-3 py-2 border-b border-border-default last:border-b-0 text-xs font-mono"
                      >
                        <span className="text-risk font-semibold">
                          {a.asset}
                        </span>
                        <span className="text-text-lo ml-2">
                          ${a.observedPrice.toFixed(4)} &lt; $
                          {a.thresholdPrice.toFixed(4)}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </div>
        ) : (
          <div className="hidden items-center gap-2 sm:flex">
            <Link
              href={authHref("/login", pathname)}
              className="inline-flex min-h-[36px] items-center justify-center gap-2 rounded-sharp border border-black bg-accent-agent px-3 text-xs font-mono font-semibold text-black shadow-brutal-sm transition-shadow hover:shadow-brutal"
            >
              <LogIn className="h-3.5 w-3.5" />
              Sign in
            </Link>
          </div>
        )}
        {sessionActive && (
          <button
            type="button"
            data-testid="header-logout-direct"
            onClick={() => void handleLogout()}
            aria-label="Log out"
            className="min-h-[36px] inline-flex items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-2.5 text-xs font-mono text-text-lo hover:border-risk/50 hover:bg-risk/5 hover:text-risk transition-colors"
          >
            <LogOut className="w-3.5 h-3.5" aria-hidden="true" />
            <span className="hidden sm:inline">Log out</span>
          </button>
        )}
        {wallet && (
          <div ref={userRef} className="relative">
            <button
              onClick={() => setUserOpen((v) => !v)}
              className="w-7 h-7 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black hover:opacity-90"
              title="Account menu"
              aria-label="Account menu"
            >
              <WalletIcon className="w-3.5 h-3.5 text-black" />
            </button>
            {userOpen && (
              <div className="absolute right-0 top-full mt-2 w-80 border-brutal border-border-default bg-surface shadow-brutal z-50 rounded-sharp">
                <div className="px-3 py-2 border-b border-border-default">
                  <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut mb-1">
                    Balances
                  </p>
                  <p className="text-sm font-mono font-semibold text-accent-pnl tabular-nums">
                    ${unifiedUsdc.toFixed(2)} USDC
                    {unifiedEurc > 0 && (
                      <span className="text-text-lo">
                        {" · "}€{unifiedEurc.toFixed(2)} EURC
                      </span>
                    )}
                  </p>
                  <p className="text-[10px] font-mono text-text-mut mt-0.5">
                    Wallet cash balance
                  </p>
                </div>
                <Link
                  href="/wallets"
                  onClick={() => setUserOpen(false)}
                  className="flex items-center gap-2 w-full text-left px-3 py-2 text-sm font-mono text-accent-agent hover:bg-raised"
                >
                  <WalletIcon className="w-3 h-3" />
                  Open wallet
                </Link>
                <button
                  type="button"
                  data-testid="header-logout"
                  onClick={() => void handleLogout()}
                  className="flex items-center gap-2 w-full border-t border-border-default px-3 py-2 text-left text-sm font-mono text-text-lo hover:bg-risk/5 hover:text-risk"
                >
                  <LogOut className="w-3 h-3" aria-hidden="true" />
                  Log out
                </button>
              </div>
            )}
          </div>
        )}
        {logoutError && (
          <div
            role="alert"
            className="absolute right-4 top-14 z-50 max-w-sm border border-risk/50 bg-risk/10 px-3 py-2 font-mono text-[11px] leading-relaxed text-risk shadow-brutal"
          >
            {logoutError}
          </div>
        )}
      </div>
    </header>
  );
}

function authHref(path: "/login", next: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

function portfolioSubtitle(
  portfolio: Pick<Portfolio, "goal"> & Partial<Pick<Portfolio, "allocations">>,
) {
  const goal = portfolio.goal;
  const risk = goal?.riskTolerance ?? "target";
  const horizon = goal?.horizon ?? "draft";
  const hydratedAssets = portfolio.allocations?.length ?? 0;
  const targetAssets = goal?.targetAllocation
    ? Object.values(goal.targetAllocation).filter((v) => (v ?? 0) > 0).length
    : 0;
  const assets = hydratedAssets || targetAssets;
  return `${risk} · ${horizon} · ${assets} assets`;
}

function displayPortfolioName(portfolio: Pick<Portfolio, "name">) {
  return portfolio.name;
}

function formatCompactUsd(value: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}
