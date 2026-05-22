"use client";

import Link from "next/link";
import { TrendingUp, TrendingDown, Wallet, ArrowRight } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { ProvenanceLine, Skeleton } from "@aegis/ui";

/// EURC's mid-market USD price for the Total Wealth headline. Cheap stable
/// approximation — the FX module's authoritative rate is read elsewhere.
const EURC_USD_APPROX = 1.085;

export function PortfolioSummaryCard() {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);

  if (!portfolio) {
    return (
      <Card className="col-span-1">
        <CardContent className="p-5">
          <Skeleton height="h-24" />
        </CardContent>
      </Card>
    );
  }

  const ageMs = snapshot
    ? Date.now() - new Date(snapshot.capturedAt).getTime()
    : 0;
  const isStale = ageMs > 60_000;
  const isVeryStale = ageMs > 300_000;
  const priceColor = isVeryStale
    ? "text-risk"
    : isStale
      ? "text-warn"
      : "text-text-hi";

  const isPositive = portfolio.totalPnlUsd >= 0;
  const TrendIcon = isPositive ? TrendingUp : TrendingDown;

  // "Total Value" is the sum of invested positions + idle wallet cash —
  // otherwise a freshly funded user sees $0 across the board and concludes
  // the platform is broken. Prefer the live EURC price off the market
  // snapshot when available; fall back to a stable ~1.085 mid otherwise.
  const eurcUsd =
    snapshot?.assets.find((a) => a.symbol === "EURC")?.priceUsd ??
    EURC_USD_APPROX;
  const idleCashUsd = unifiedUsdc + unifiedEurc * eurcUsd;
  const positionMetrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const investedUsd = positionMetrics.investedUsd;
  const walletBalanceUnavailable = gatewayBalanceStatus === "error";
  const walletBalanceLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const confirmedIdleCashUsd = walletBalanceUnavailable ? 0 : idleCashUsd;
  const totalWealthUsd = investedUsd + confirmedIdleCashUsd;

  const arcTotal = (perChainUsdc.arc ?? 0) + (perChainEurc.arc ?? 0) * eurcUsd;
  const baseTotal =
    (perChainUsdc.base ?? 0) + (perChainEurc.base ?? 0) * eurcUsd;

  return (
    <Card data-testid="portfolio-summary">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5" />
          Net Worth
        </CardTitle>
      </CardHeader>
      <CardContent>
        <p className={`text-3xl font-bold mb-1 ${priceColor}`}>
          {formatCurrency(totalWealthUsd)}
        </p>
        <p className="text-[11px] font-mono text-text-mut mb-3">
          {formatCurrency(investedUsd, { compact: true })} invested{" · "}
          {walletBalanceUnavailable
            ? "wallet balance unavailable"
            : walletBalanceLoading
              ? "checking wallet balance"
              : `${formatCurrency(idleCashUsd, { compact: true })} in wallet`}
        </p>
        {investedUsd > 0.5 ? (
          <div
            className={`flex items-center gap-1.5 text-sm ${changeColor(portfolio.totalPnlUsd)}`}
          >
            <TrendIcon className="w-3.5 h-3.5" />
            <span className="font-medium">
              {formatCurrency(portfolio.totalPnlUsd, { compact: true })}
            </span>
            <span className="text-text-mut">·</span>
            <span>{formatPercent(portfolio.totalPnlPct)}</span>
            <span className="text-text-mut text-xs ml-1">all time</span>
          </div>
        ) : (
          <p className="text-[11px] font-mono text-text-mut">
            No investment yet. PnL appears after your first approved move.
          </p>
        )}

        {!walletBalanceUnavailable &&
          !walletBalanceLoading &&
          idleCashUsd > 0.5 && (
            <div className="mt-4 grid grid-cols-2 gap-2 text-[11px] font-mono">
              <div className="p-2 rounded-sharp bg-raised border border-border-default">
                <p className="text-text-mut uppercase tracking-wider text-[9px] mb-0.5">
                  Arc balance
                </p>
                <p className="text-text-hi tabular-nums">
                  {formatCurrency(arcTotal, { compact: true })}
                </p>
              </div>
              <div className="p-2 rounded-sharp bg-raised border border-border-default">
                <p className="text-text-mut uppercase tracking-wider text-[9px] mb-0.5">
                  Base balance
                </p>
                <p className="text-text-hi tabular-nums">
                  {formatCurrency(baseTotal, { compact: true })}
                </p>
              </div>
              <Link
                href="/wallets"
                className="col-span-2 flex items-center justify-between px-2 py-1.5 rounded-sharp text-[10px] text-accent-pnl/80 hover:text-accent-pnl hover:bg-accent-pnl/5 transition-colors"
              >
                <span>Wallet address + per-token breakdown</span>
                <ArrowRight className="w-3 h-3" />
              </Link>
            </div>
          )}

        <div className="mt-4 grid grid-cols-2 gap-3">
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-xs text-text-mut mb-1">Assets</p>
            <p className="text-sm font-semibold text-text-hi">
              {portfolio.allocations?.length ?? 0}
            </p>
          </div>
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-xs text-text-mut mb-1">Risk Score</p>
            {investedUsd > 0.5 ? (
              <>
                <div className="flex items-center gap-2">
                  <p
                    className={`text-sm font-semibold ${portfolio.riskScore < 40 ? "text-accent-agent" : portfolio.riskScore < 65 ? "text-warn" : "text-risk"}`}
                  >
                    {portfolio.riskScore}/100
                  </p>
                  {isVeryStale && (
                    <span className="text-risk text-[10px]">stale</span>
                  )}
                  {isStale && !isVeryStale && (
                    <span className="text-warn text-[10px]">stale</span>
                  )}
                </div>
                {snapshot && (
                  <p className="text-[10px] text-text-mut mt-0.5">
                    as of{" "}
                    {new Date(snapshot.capturedAt).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </p>
                )}
              </>
            ) : (
              <>
                <p className="text-sm font-semibold text-text-mut">—</p>
                <p className="text-[10px] text-text-mut mt-0.5">
                  available after first approved move
                </p>
              </>
            )}
          </div>

          <div className="col-span-2 pt-2 border-t border-white/10">
            <ProvenanceLine
              source={
                walletBalanceUnavailable
                  ? "confirmed positions · wallet balance check failed"
                  : walletBalanceLoading
                    ? "confirmed positions · wallet balance warming up"
                    : positionMetrics.usingLivePrices
                      ? "wallet cash + live position marks"
                      : "wallet cash + confirmed positions"
              }
              freshness={walletBalanceUnavailable ? "needs retry" : "live"}
              className={walletBalanceUnavailable ? "text-warn" : undefined}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
