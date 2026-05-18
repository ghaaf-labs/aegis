"use client";

import { TrendingUp, TrendingDown } from "lucide-react";
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

  if (!portfolio) return null;

  const ageMs = snapshot
    ? Date.now() - new Date(snapshot.capturedAt).getTime()
    : 0;
  const isStale = ageMs > 60_000;
  const isVeryStale = ageMs > 300_000;
  const priceColor = isVeryStale
    ? "text-red-400"
    : isStale
      ? "text-yellow-400"
      : "text-white";

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

  // Compute live current weight per allocation from holdings × spot price.
  // The stored `currentWeight` column is initialized to the target on
  // portfolio creation and not maintained by the executor, so reading it
  // would show "50% vs 50%" even when the user holds 0 units.
  const allocList = portfolio.allocations ?? [];
  const investedValues = allocList.map((a) => {
    const price = priceMap[a.symbol]?.priceUsd ?? 0;
    return price * a.quantity;
  });
  const totalInvestedUsd = investedValues.reduce((sum, v) => sum + v, 0);
  const isUninvested = totalInvestedUsd < 0.5;
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
        <CardTitle>Holdings</CardTitle>
      </CardHeader>
      <CardContent className="p-0 overflow-x-auto">
        <table className="w-full min-w-[640px]">
          <thead>
            <tr className="border-b border-white/5">
              {(
                [
                  ["Asset", ""],
                  ["Price", ""],
                  ["24h", "hidden sm:table-cell"],
                  ["Holdings", "hidden md:table-cell"],
                  ["Value", ""],
                  ["Weight vs Target", "hidden lg:table-cell"],
                ] as const
              ).map(([h, cls]) => (
                <th
                  key={h}
                  className={
                    "px-5 py-3 text-left text-[11px] font-medium text-gray-500 uppercase tracking-wider " +
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
              const valueUsd = (price?.priceUsd ?? 0) * alloc.quantity;
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
                      <div className="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500/30 to-violet-500/30 flex items-center justify-center border border-white/10">
                        <span className="text-[10px] font-bold text-white">
                          {alloc.symbol[0]}
                        </span>
                      </div>
                      <span className="text-sm font-semibold text-white font-mono">
                        {alloc.symbol}
                      </span>
                    </div>
                  </td>
                  <td
                    className={`px-5 py-3.5 text-sm font-medium ${priceColor}`}
                  >
                    {price ? formatCurrency(price.priceUsd) : "—"}
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
                  <td className="px-5 py-3.5 text-xs text-gray-400 font-mono hidden md:table-cell">
                    {formatNumber(alloc.quantity)}
                  </td>
                  <td className="px-5 py-3.5 text-sm text-white font-medium">
                    {formatCurrency(valueUsd)}
                  </td>
                  <td className="px-5 py-3.5 hidden lg:table-cell">
                    {isUninvested ? (
                      <span className="text-xs text-gray-500 font-mono">
                        target {alloc.targetWeight.toFixed(0)}%
                      </span>
                    ) : (
                      <div className="flex items-center gap-2">
                        <div className="flex items-center gap-1">
                          <span className="text-xs text-gray-400 font-mono w-10">
                            {currentWeight.toFixed(1)}%
                          </span>
                          <span className="text-gray-600 text-xs">vs</span>
                          <span className="text-xs text-gray-500 font-mono w-10">
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
                </tr>
              );
            })}
          </tbody>
        </table>
        <div className="px-5 py-2 text-[10px] text-text-mut font-mono border-t border-white/5">
          {snapshot
            ? "Prices via DefiLlama · live snapshot"
            : "Awaiting first price tick · via DefiLlama"}
        </div>
      </CardContent>
    </Card>
  );
}
