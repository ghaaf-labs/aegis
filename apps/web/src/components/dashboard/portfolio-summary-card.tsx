"use client";

import { TrendingUp, TrendingDown, Wallet, CircleAlert } from "lucide-react";
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
  return (
    <Card data-testid="portfolio-summary">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5" />
          Total Value
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div>
          <p className={`text-2xl font-bold leading-none ${priceColor}`}>
            {formatCurrency(totalWealthUsd)}
          </p>
          <div className="mt-3 grid gap-2 font-mono text-[11px]">
            <BreakdownRow
              label="Invested"
              value={formatCurrency(investedUsd, { compact: true })}
            />
            <BreakdownRow
              label="Wallet cash"
              value={
                walletBalanceUnavailable
                  ? "unknown"
                  : walletBalanceLoading
                    ? "checking"
                    : formatCurrency(idleCashUsd, { compact: true })
              }
              tone={
                walletBalanceUnavailable
                  ? "warn"
                  : walletBalanceLoading
                    ? "muted"
                    : "default"
              }
            />
          </div>
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
          <div className="flex items-start gap-2 border border-warn/45 bg-warn/5 px-3 py-2 font-mono">
            <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warn" />
            <div>
              <p className="text-xs font-semibold text-warn">
                Waiting for first approval
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                Wallet cash is counted, but invested value starts after you
                approve a plan.
              </p>
            </div>
          </div>
        )}

        <div className="border-t border-white/10 pt-2">
          <ProvenanceLine
            source={
              walletBalanceUnavailable
                ? "wallet cash check failed"
                : walletBalanceLoading
                  ? "wallet cash warming up"
                  : positionMetrics.usingLivePrices
                    ? "wallet cash + live marks"
                    : "wallet cash + confirmed positions"
            }
            freshness={walletBalanceUnavailable ? "needs retry" : "live"}
            className={walletBalanceUnavailable ? "text-warn" : undefined}
          />
        </div>
      </CardContent>
    </Card>
  );
}

function BreakdownRow({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "muted" | "warn";
}) {
  const valueClass =
    tone === "warn"
      ? "text-warn"
      : tone === "muted"
        ? "text-text-lo"
        : "text-text-hi";

  return (
    <div className="grid min-h-8 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-border-default bg-bg/70 px-2.5 py-1.5">
      <span className="text-text-mut">{label}</span>
      <span className={`font-semibold tabular-nums ${valueClass}`}>
        {value}
      </span>
    </div>
  );
}
