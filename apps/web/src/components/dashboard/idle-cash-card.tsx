"use client";

import Link from "next/link";
import { Wallet, ArrowRight } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";
import { ProvenanceLine } from "@aegis/ui";

/**
 * First-class dashboard card for Circle Gateway idle cash. Replaces the
 * tiny "GATEWAY $X USDC · €Y EURC" string in the page header — that
 * string was easy to miss and hidden on mobile. Per-chain breakdown
 * (Arc vs Base) is also surfaced so the user can see where each stable
 * lives without leaving the dashboard.
 */
export function IdleCashCard() {
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

  const eurcUsd =
    snapshot?.assets.find((a) => a.symbol === "EURC")?.priceUsd ?? 1.085;
  const totalUsd = unifiedUsdc + unifiedEurc * eurcUsd;
  const hasIdleCash = totalUsd > 0.5;

  const arcTotal = (perChainUsdc.arc ?? 0) + (perChainEurc.arc ?? 0) * eurcUsd;
  const baseTotal =
    (perChainUsdc.base ?? 0) + (perChainEurc.base ?? 0) * eurcUsd;

  return (
    <Card data-testid="idle-cash-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5 text-accent-pnl" />
          Wallet Balance
        </CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-3xl font-bold text-text-hi mb-1">
          {formatCurrency(totalUsd)}
        </p>
        <p className="text-[11px] font-mono text-text-mut mb-4">
          {hasIdleCash ? (
            <>
              {formatCurrency(unifiedUsdc, { compact: true })} USDC
              {unifiedEurc > 0 && (
                <>
                  {" · "}€{unifiedEurc.toFixed(2)} EURC
                </>
              )}{" "}
              undeployed
            </>
          ) : (
            "No deployable cash in Gateway right now"
          )}
        </p>

        <div className="grid grid-cols-2 gap-3">
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
              Arc
            </p>
            <p className="text-sm font-semibold text-text-hi tabular-nums">
              {formatCurrency(arcTotal, { compact: true })}
            </p>
            <p className="text-[10px] text-text-lo font-mono mt-0.5">
              {(perChainUsdc.arc ?? 0).toFixed(2)} USDC
              {(perChainEurc.arc ?? 0) > 0 && (
                <> · €{(perChainEurc.arc ?? 0).toFixed(2)}</>
              )}
            </p>
          </div>
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
              Base
            </p>
            <p className="text-sm font-semibold text-text-hi tabular-nums">
              {formatCurrency(baseTotal, { compact: true })}
            </p>
            <p className="text-[10px] text-text-lo font-mono mt-0.5">
              {(perChainUsdc.base ?? 0).toFixed(2)} USDC
              {(perChainEurc.base ?? 0) > 0 && (
                <> · €{(perChainEurc.base ?? 0).toFixed(2)}</>
              )}
            </p>
          </div>
        </div>

        <Link
          href="/wallet"
          className="mt-4 flex min-h-10 items-center justify-between gap-3 px-3 py-2 rounded-sharp bg-accent-pnl/5 border border-accent-pnl/20 text-xs font-mono text-accent-pnl hover:bg-accent-pnl/10 transition-colors"
        >
          <span>
            {hasIdleCash
              ? "Wallet details + addresses"
              : "Add funds or inspect addresses"}
          </span>
          <ArrowRight className="w-3.5 h-3.5" />
        </Link>

        <div className="mt-3 pt-2 border-t border-white/10">
          <ProvenanceLine
            source="Circle Gateway · unified balance"
            freshness="live"
          />
        </div>
      </CardContent>
    </Card>
  );
}
