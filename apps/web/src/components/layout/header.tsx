"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { ChevronDown, Bell, Plus, Wifi, WifiOff } from "lucide-react";
import { BrutalButton, BrutalPill, ChainBadge } from "@aegis/ui";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";
import { gatewayApi } from "@/lib/api";

export function Header() {
  const router = useRouter();
  const portfolio = useActivePortfolio();
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const setActive = usePortfolioStore((s) => s.setActivePortfolio);
  const sseConnected = usePortfolioStore((s) => s.sseConnected);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const wallet = usePortfolioStore((s) => s.wallet);

  const [open, setOpen] = useState(false);

  // Fetch unified balance on first render — Gateway SSE keeps it fresh after.
  useEffect(() => {
    if (!wallet) return;
    let alive = true;
    void gatewayApi
      .balance()
      .then((b) => alive && setUnifiedUsdc(b.unifiedUsdc))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [wallet, setUnifiedUsdc]);

  return (
    <header className="flex items-center justify-between px-6 py-3 border-b-brutal border-border-default bg-surface shrink-0">
      <div className="flex items-center gap-4">
        {portfolio && (
          <div className="relative">
            <button
              onClick={() => setOpen((v) => !v)}
              className="flex items-center gap-2 px-3 py-1.5 border-brutal border-border-default rounded-sharp bg-bg hover:border-border-hi"
            >
              <span className="text-xs text-text-lo font-mono">Portfolio</span>
              <span className="text-sm font-semibold text-text-hi">
                {portfolio.name}
              </span>
              <ChevronDown className="w-3 h-3 text-text-lo" />
            </button>
            {open && (
              <div className="absolute top-full left-0 mt-2 w-64 border-brutal border-border-default bg-surface shadow-brutal rounded-card z-50">
                {portfolios.map((p) => (
                  <button
                    key={p.id}
                    onClick={() => {
                      setActive(p.id);
                      setOpen(false);
                      router.push(`/dashboard/${p.id}`);
                    }}
                    className={`block w-full text-left px-3 py-2 text-sm font-mono border-b border-border-default last:border-b-0 hover:bg-raised ${
                      p.id === portfolio.id
                        ? "text-accent-pnl"
                        : "text-text-default"
                    }`}
                  >
                    {p.name}
                    <span className="ml-2 text-text-mut text-xs">
                      {formatCurrency(p.totalValueUsd)}
                    </span>
                  </button>
                ))}
                <button
                  onClick={() => {
                    setOpen(false);
                    router.push("/onboarding");
                  }}
                  className="flex items-center gap-2 w-full text-left px-3 py-2 text-sm font-mono text-accent-agent hover:bg-raised"
                >
                  <Plus className="w-3 h-3" />
                  New portfolio
                </button>
              </div>
            )}
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
            {wallet && (
              <div>
                <p className="text-[10px] text-text-mut font-mono">
                  GATEWAY USDC
                  <span className="ml-1 inline-flex gap-1">
                    <ChainBadge chain="ARC" />
                    <ChainBadge chain="BASE" />
                  </span>
                </p>
                <p className="text-sm font-mono font-semibold text-accent-pnl tabular-nums">
                  ${unifiedUsdc.toFixed(2)}
                </p>
              </div>
            )}
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
        <BrutalButton
          variant="ghost"
          className="text-text-lo"
          aria-label="Notifications"
        >
          <Bell className="w-4 h-4" />
        </BrutalButton>
        {wallet && (
          <div
            className="w-7 h-7 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black"
            title={wallet.arcAddress}
          >
            <span className="text-xs font-mono font-semibold text-black">
              {wallet.walletId.slice(-2).toUpperCase()}
            </span>
          </div>
        )}
      </div>
    </header>
  );
}
