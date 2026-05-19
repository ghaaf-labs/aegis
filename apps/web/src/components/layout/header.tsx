"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import {
  Bell,
  Wifi,
  WifiOff,
  LogOut,
  Wallet as WalletIcon,
} from "lucide-react";
import { BrutalButton, BrutalPill } from "@aegis/ui";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";
import { gatewayApi, walletApi } from "@/lib/api";

export function Header() {
  const router = useRouter();
  const portfolio = useActivePortfolio();
  const sseConnected = usePortfolioStore((s) => s.sseConnected);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const wallet = usePortfolioStore((s) => s.wallet);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);

  const [notifOpen, setNotifOpen] = useState(false);
  const [userOpen, setUserOpen] = useState(false);
  const notifRef = useRef<HTMLDivElement>(null);
  const userRef = useRef<HTMLDivElement>(null);
  const pegAlerts = usePortfolioStore((s) => s.pegAlerts);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (notifRef.current && !notifRef.current.contains(e.target as Node))
        setNotifOpen(false);
      if (userRef.current && !userRef.current.contains(e.target as Node))
        setUserOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const handleLogout = async () => {
    setUserOpen(false);
    try {
      await walletApi.logout();
    } catch {
      // Backend already-401 is fine — we just need to clear local state.
    }
    setWallet(null);
    setPortfolios([]);
    setUnifiedUsdc(0);
    setUnifiedEurc(0);
    localStorage.removeItem("aegis_email");
    router.push("/login");
  };

  // Fetch unified balance on first render — Gateway SSE keeps it fresh after.
  useEffect(() => {
    if (!wallet) return;
    let alive = true;
    void gatewayApi
      .balance()
      .then((b) => {
        if (!alive) return;
        setUnifiedUsdc(b.unifiedUsdc);
        setUnifiedEurc(b.unifiedEurc);
        setPerChain(b.perChain ?? {}, b.perChainEurc ?? {});
      })
      .catch((err) => {
        // Best-effort hydration; SSE will overwrite this once the channel
        // opens. Surface in dev so we can spot persistent gateway outages.
        if (alive) console.warn("gateway balance hydrate failed", err);
      });
    return () => {
      alive = false;
    };
  }, [wallet, setUnifiedUsdc]);

  return (
    <header className="flex items-center justify-between px-6 py-3 border-b-brutal border-border-default bg-surface shrink-0">
      <div className="flex items-center gap-4">
        {portfolio && (
          <div className="flex items-center gap-2 px-3 py-1.5 border-brutal border-border-default rounded-sharp bg-bg">
            <span className="text-xs text-text-lo font-mono">Portfolio</span>
            <span className="text-sm font-semibold text-text-hi">
              {portfolio.name}
            </span>
          </div>
        )}

        {portfolio && (
          <div className="hidden md:flex items-center gap-6 ml-2">
            <div>
              <p className="text-[10px] text-text-mut font-mono">
                PORTFOLIO VALUE
              </p>
              <p className="text-sm font-mono font-semibold text-text-hi tabular-nums">
                {formatCurrency(portfolio.totalValueUsd)}
              </p>
            </div>
            {portfolio.totalValueUsd > 0.5 && (
              <div>
                <p className="text-[10px] text-text-mut font-mono">
                  ALL-TIME PNL
                </p>
                <p
                  className={`text-sm font-mono font-semibold tabular-nums ${changeColor(portfolio.totalPnlUsd)}`}
                >
                  {formatCurrency(portfolio.totalPnlUsd)} (
                  {formatPercent(portfolio.totalPnlPct)})
                </p>
              </div>
            )}
            {/* Wallet balance + per-chain breakdown lives on the Net Worth
                card now — keeping a duplicate here clutters the header. */}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3 ml-auto">
        <BrutalPill tone={sseConnected ? "pnl" : "neutral"}>
          {sseConnected ? (
            <Wifi className="w-3 h-3" />
          ) : (
            <WifiOff className="w-3 h-3" />
          )}
          <span>{sseConnected ? "LIVE" : "OFFLINE"}</span>
        </BrutalPill>
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
                      <span className="text-risk font-semibold">{a.asset}</span>
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
                    Arc + Base · Circle Gateway
                  </p>
                </div>
                <Link
                  href="/wallet"
                  onClick={() => setUserOpen(false)}
                  className="flex items-center gap-2 w-full text-left px-3 py-2 text-sm font-mono text-accent-agent hover:bg-raised"
                >
                  <WalletIcon className="w-3 h-3" />
                  Open wallet
                </Link>
                <button
                  onClick={() => void handleLogout()}
                  className="flex items-center gap-2 w-full text-left px-3 py-2 text-sm font-mono text-risk hover:bg-raised border-t border-border-default"
                >
                  <LogOut className="w-3 h-3" />
                  Log out
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </header>
  );
}
