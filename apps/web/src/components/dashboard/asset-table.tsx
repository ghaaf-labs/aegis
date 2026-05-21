"use client";

import Link from "next/link";
import { ArrowRight, TrendingUp, TrendingDown, Wallet } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import {
  formatCurrency,
  formatPercent,
  formatNumber,
  changeColor,
} from "@/lib/utils";

export function AssetTable() {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const livePrices = usePortfolioStore((s) => s.livePrices);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  // Live-tick source is the truthful provenance — FallbackProvider switches
  // between defillama and pyth dynamically, hardcoding either name lies
  // whenever the breaker is open or a single primary call failed.
  const liveSource = Object.values(livePrices)[0]?.source;

  if (!portfolio) return null;

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

  // USYC is a tokenized USD instrument from Hashnote — not on DefiLlama's
  // free feed, so the snapshot omits it. Fall back to $1.00 (the floor;
  // accrued yield is small relative to the dashboard scale) so the row
  // doesn't read as "broken price feed".
  const snapshotAssets = snapshot?.assets ?? [];
  const priceMap = Object.fromEntries(
    snapshotAssets.map((a) => [a.symbol, a]),
  ) as Record<string, (typeof snapshotAssets)[number] | undefined>;
  if (!priceMap.USYC) {
    priceMap.USYC = {
      symbol: "USYC",
      priceUsd: 1.0,
      change24h: 0,
      change7d: 0,
      marketCap: 0,
      volume24h: 0,
      updatedAt: snapshot?.capturedAt ?? new Date().toISOString(),
    };
  }
  const deployableUsd = unifiedUsdc;

  // Compute live current weight per allocation from holdings × spot price.
  // The stored `currentWeight` column is initialized to the target on
  // portfolio creation and not maintained by the executor, so reading it
  // would show "50% vs 50%" even when the user holds 0 units.
  const allocList = portfolio.allocations ?? [];
  const investedValues = allocList.map((a) => {
    const liveValue = (priceMap[a.symbol]?.priceUsd ?? 0) * a.quantity;
    return liveValue > 0 ? liveValue : a.valueUsd;
  });
  const derivedInvestedUsd = investedValues.reduce((sum, v) => sum + v, 0);
  const totalInvestedUsd =
    derivedInvestedUsd > 0.5 ? derivedInvestedUsd : portfolio.totalValueUsd;
  const isUninvested = totalInvestedUsd < 0.5;
  const hasDeployableWallet = isUninvested && deployableUsd > 0.5;
  const liveWeights: Record<string, number> = Object.fromEntries(
    allocList.map((a, i) => [
      a.symbol,
      totalInvestedUsd > 0
        ? ((investedValues[i] ?? 0) / totalInvestedUsd) * 100
        : 0,
    ]),
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          {hasDeployableWallet
            ? "Target Deployment Plan"
            : "Invested Positions"}
        </CardTitle>
      </CardHeader>
      {hasDeployableWallet && (
        <div className="mx-5 mb-4 border-brutal border-accent-pnl/40 bg-accent-pnl/5 p-3 rounded-sharp">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-start gap-2">
              <Wallet className="mt-0.5 h-4 w-4 shrink-0 text-accent-pnl" />
              <div>
                <p className="text-sm font-mono font-semibold text-text-hi">
                  {formatCurrency(deployableUsd)} USDC is still in your wallet,
                  not invested yet.
                </p>
                <p className="mt-1 text-xs font-mono text-text-lo leading-relaxed">
                  The rows below show how that USDC balance would be split after
                  you approve a deploy plan. Current position value is zero
                  until the rebalance legs confirm.
                  {unifiedEurc > 0 &&
                    " EURC stays in Wallet until StableFX deployment is enabled."}
                </p>
              </div>
            </div>
            <Link
              href={`/dashboard/${portfolio.id}`}
              className="inline-flex shrink-0 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-pnl px-3 py-2 text-xs font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal transition-[box-shadow]"
            >
              Deploy from dashboard
              <ArrowRight className="h-3.5 w-3.5" />
            </Link>
          </div>
        </div>
      )}
      <CardContent className="p-0 overflow-x-auto">
        <table className="w-full min-w-[640px]">
          <thead>
            <tr className="border-b border-white/5">
              {(hasDeployableWallet
                ? ([
                    ["Asset", ""],
                    ["Target", ""],
                    ["Planned from wallet", ""],
                    ["Current holdings", "hidden md:table-cell"],
                    ["Status", "hidden lg:table-cell"],
                  ] as const)
                : ([
                    ["Asset", ""],
                    ["Price", ""],
                    ["24h", "hidden sm:table-cell"],
                    ["Holdings", "hidden md:table-cell"],
                    ["Value", ""],
                    ["Weight vs Target", "hidden lg:table-cell"],
                  ] as const)
              ).map(([h, cls]) => (
                <th
                  key={h}
                  className={
                    "px-5 py-3 text-left text-[11px] font-medium text-text-mut uppercase tracking-wider " +
                    cls
                  }
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(portfolio.allocations ?? []).map((alloc, i) => {
              const price = priceMap[alloc.symbol];
              const currentWeight = liveWeights[alloc.symbol] ?? 0;
              const liveValueUsd = (price?.priceUsd ?? 0) * alloc.quantity;
              const valueUsd = liveValueUsd > 0 ? liveValueUsd : alloc.valueUsd;
              const fallbackPriceUsd =
                alloc.quantity > 0 && alloc.valueUsd > 0
                  ? alloc.valueUsd / alloc.quantity
                  : null;
              const displayPriceUsd = price?.priceUsd ?? fallbackPriceUsd;
              const plannedUsd = deployableUsd * (alloc.targetWeight / 100);
              const drift = currentWeight - alloc.targetWeight;
              const driftAbs = Math.abs(drift);

              return (
                <tr
                  key={alloc.symbol}
                  className={`border-b border-white/3 hover:bg-white/2 transition-colors ${
                    i === (portfolio.allocations?.length ?? 0) - 1
                      ? "border-0"
                      : ""
                  }`}
                >
                  <td className="px-5 py-3.5">
                    <div className="flex items-center gap-2.5">
                      <div className="w-7 h-7 rounded-sharp bg-accent-agent/10 flex items-center justify-center border border-accent-agent/30">
                        <span className="text-[10px] font-bold text-text-hi">
                          {alloc.symbol[0]}
                        </span>
                      </div>
                      <span className="text-sm font-semibold text-text-hi font-mono">
                        {alloc.symbol}
                      </span>
                    </div>
                  </td>
                  {hasDeployableWallet ? (
                    <>
                      <td className="px-5 py-3.5 text-sm font-mono text-text-hi tabular-nums">
                        {alloc.targetWeight.toFixed(0)}%
                      </td>
                      <td className="px-5 py-3.5 text-sm font-semibold text-accent-pnl tabular-nums">
                        {formatCurrency(plannedUsd)}
                      </td>
                      <td className="px-5 py-3.5 text-xs text-text-lo font-mono hidden md:table-cell">
                        {formatNumber(alloc.quantity)}
                      </td>
                      <td className="px-5 py-3.5 hidden lg:table-cell">
                        <span className="text-xs text-text-mut font-mono">
                          awaiting approval
                        </span>
                      </td>
                    </>
                  ) : (
                    <>
                      <td
                        className={`px-5 py-3.5 text-sm font-medium ${priceColor}`}
                      >
                        {displayPriceUsd
                          ? formatCurrency(displayPriceUsd)
                          : "—"}
                      </td>
                      <td className="px-5 py-3.5 hidden sm:table-cell">
                        {price && (
                          <span
                            className={`text-xs flex items-center gap-1 ${changeColor(price.change24h)}`}
                          >
                            {price.change24h >= 0 ? (
                              <TrendingUp className="w-3 h-3" />
                            ) : (
                              <TrendingDown className="w-3 h-3" />
                            )}
                            {formatPercent(price.change24h)}
                          </span>
                        )}
                      </td>
                      <td className="px-5 py-3.5 text-xs text-text-lo font-mono hidden md:table-cell">
                        {formatNumber(alloc.quantity)}
                      </td>
                      <td className="px-5 py-3.5 text-sm text-text-hi font-medium">
                        {formatCurrency(valueUsd)}
                      </td>
                      <td className="px-5 py-3.5 hidden lg:table-cell">
                        {isUninvested ? (
                          <span className="text-xs text-text-mut font-mono">
                            target {alloc.targetWeight.toFixed(0)}%
                          </span>
                        ) : (
                          <div className="flex items-center gap-2">
                            <div className="flex items-center gap-1">
                              <span className="text-xs text-text-lo font-mono w-10">
                                {currentWeight.toFixed(1)}%
                              </span>
                              <span className="text-text-mut text-xs">vs</span>
                              <span className="text-xs text-text-mut font-mono w-10">
                                {alloc.targetWeight.toFixed(0)}%
                              </span>
                            </div>
                            {driftAbs > 3 && (
                              <Badge
                                variant={driftAbs > 10 ? "danger" : "warning"}
                                className="text-[10px] px-1.5 py-0"
                              >
                                {drift > 0 ? "+" : ""}
                                {drift.toFixed(1)}%
                              </Badge>
                            )}
                          </div>
                        )}
                      </td>
                    </>
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
        <div className="px-5 py-2 text-[10px] text-text-mut font-mono border-t border-white/5">
          {hasDeployableWallet
            ? "Planned values use deployable Circle Gateway USDC × target weights"
            : snapshot
              ? `Prices via ${liveSource ?? "live feed"} · live snapshot`
              : "Values use last confirmed holdings while live prices warm up"}
        </div>
      </CardContent>
    </Card>
  );
}
