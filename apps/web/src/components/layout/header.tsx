"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { Bell, LogIn, Wifi, WifiOff, Wallet as WalletIcon } from "lucide-react";
import { BrutalButton, BrutalPill } from "@aegis/ui";
import { usePortfolioStore } from "@/stores/portfolio";
import { gatewayApi } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";

export function Header() {
  const pathname = usePathname();
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
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const walletPending = sessionActive && !wallet;

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
        setPerChain(
          b.perChain ?? {},
          b.perChainEurc ?? {},
          undefined,
          b.tokenBalancesByChain ?? {},
        );
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
    <header className="flex h-14 shrink-0 items-center justify-between border-b-brutal border-border-default bg-surface px-5">
      <div aria-hidden="true" />

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
            className="touch-target inline-flex min-h-11 items-center justify-center gap-2 rounded-sharp border border-warn/40 bg-warn/5 px-3 text-[10px] font-mono uppercase tracking-widest text-warn transition-colors hover:bg-warn/10"
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
              aria-haspopup="menu"
              aria-expanded={notifOpen}
              onClick={() => setNotifOpen((v) => !v)}
            >
              <Bell className="w-4 h-4" />
              {pegAlerts.length > 0 && (
                <span className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-sharp bg-risk" />
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
              className="touch-target inline-flex min-h-11 items-center justify-center gap-2 rounded-sharp border border-black bg-accent-agent px-3 text-xs font-mono font-semibold text-black shadow-brutal-sm transition-shadow hover:shadow-brutal"
            >
              <LogIn className="h-3.5 w-3.5" />
              Sign in
            </Link>
          </div>
        )}
        {wallet && (
          <div ref={userRef} className="relative">
            <button
              onClick={() => setUserOpen((v) => !v)}
              className="touch-target flex h-11 w-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent hover:opacity-90"
              title="Wallet menu"
              aria-label="Wallet menu"
              aria-haspopup="menu"
              aria-expanded={userOpen}
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
              </div>
            )}
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
