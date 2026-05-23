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
  const currentHoldingCount = positionMetrics.positions.filter(
    (position) => position.valueUsd > 0.5,
  ).length;
  const targetAssetCount = portfolio.allocations.filter(
    (allocation) => allocation.targetWeight > 0,
  ).length;

  return (
    <Card data-testid="portfolio-summary" className="h-full">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5" />
          Net Worth
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div>
          <p
            className={`text-2xl font-bold leading-none sm:text-3xl ${priceColor}`}
          >
            {formatCurrency(totalWealthUsd)}
          </p>
          <p className="mt-1 font-mono text-[11px] text-text-mut">
            {formatCurrency(investedUsd, { compact: true })} invested{" · "}
            {walletBalanceUnavailable
              ? "wallet balance unavailable"
              : walletBalanceLoading
                ? "checking wallet balance"
                : `${formatCurrency(idleCashUsd, { compact: true })} wallet cash`}
          </p>
        </div>
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
          <div className="border border-border-default bg-bg/70 px-3 py-2 font-mono">
            <p className="text-xs font-semibold text-text-hi">
              Not invested yet
            </p>
            <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
              Value appears here after you approve and execute the first plan.
            </p>
          </div>
        )}

        {!walletBalanceUnavailable &&
          !walletBalanceLoading &&
          idleCashUsd > 0.5 && (
            <div className="grid grid-cols-2 gap-2 font-mono text-[11px]">
              <div className="border border-border-default bg-bg/70 p-2">
                <p className="mb-0.5 text-[9px] uppercase tracking-wider text-text-mut">
                  Arc balance
                </p>
                <p className="text-text-hi tabular-nums">
                  {formatCurrency(arcTotal, { compact: true })}
                </p>
              </div>
              <div className="border border-border-default bg-bg/70 p-2">
                <p className="mb-0.5 text-[9px] uppercase tracking-wider text-text-mut">
                  Base balance
                </p>
                <p className="text-text-hi tabular-nums">
                  {formatCurrency(baseTotal, { compact: true })}
                </p>
              </div>
              <Link
                href="/wallets"
                className="col-span-2 flex items-center justify-between gap-2 border border-accent-pnl/20 bg-accent-pnl/5 px-2 py-1.5 text-[10px] text-accent-pnl/80 transition-colors hover:text-accent-pnl"
              >
                <span className="min-w-0 truncate">
                  Wallet address + per-token breakdown
                </span>
                <ArrowRight className="h-3 w-3 shrink-0" />
              </Link>
            </div>
          )}

        <div className="grid gap-2 border-t border-white/10 pt-2 font-mono text-[11px]">
          <div className="flex min-h-8 items-center justify-between gap-3">
            <p className="text-text-mut">Current holdings</p>
            <p className="font-semibold text-text-hi tabular-nums">
              {currentHoldingCount}
            </p>
          </div>
          <div className="flex min-h-8 items-center justify-between gap-3">
            <p className="text-text-mut">Target assets</p>
            <p className="font-semibold text-text-hi tabular-nums">
              {targetAssetCount}
            </p>
          </div>
          <div className="grid min-h-8 grid-cols-[minmax(0,1fr)_auto] items-start gap-3">
            <p className="text-text-mut">Risk score</p>
            {investedUsd > 0.5 ? (
              <div className="text-right">
                <div className="flex items-center justify-end gap-2">
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
              </div>
            ) : (
              <p className="max-w-36 text-right text-[10px] leading-snug text-text-mut">
                after first approved plan
              </p>
            )}
          </div>

          <div className="pt-2 border-t border-white/10">
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
