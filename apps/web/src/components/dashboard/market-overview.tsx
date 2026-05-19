"use client";

import { TrendingUp, TrendingDown } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";
import { ProvenanceLine } from "@aegis/ui";

export function MarketOverview() {
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const livePrices = usePortfolioStore((s) => s.livePrices);

  if (!snapshot) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Market</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 animate-pulse">
          {[...Array(3)].map((_, i) => (
            <div key={i} className="flex items-center justify-between">
              <span className="h-2.5 w-24 rounded bg-white/10" />
              <span className="h-2.5 w-16 rounded bg-white/10" />
            </div>
          ))}
          <div className="border-t border-white/5 pt-3 space-y-2">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="flex items-center justify-between">
                <span className="h-2 w-10 rounded bg-white/10" />
                <span className="h-2 w-20 rounded bg-white/10" />
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    );
  }

  // Overlay live SSE ticks on top of the snapshot capture. The snapshot is the
  // historical anchor; live ticks freshen the price + 24h delta as they arrive.
  const newestTickAt = Object.values(livePrices)
    .map((t) => new Date(t.fetchedAt).getTime())
    .reduce((max, t) => (t > max ? t : max), 0);
  const effectiveCapturedAt =
    newestTickAt > new Date(snapshot.capturedAt).getTime()
      ? new Date(newestTickAt).toISOString()
      : snapshot.capturedAt;
  const ageMs = Date.now() - new Date(effectiveCapturedAt).getTime();
  const isStale = ageMs > 60_000; // > 1 minute
  const isVeryStale = ageMs > 300_000; // > 5 minutes
  const priceColor = isVeryStale
    ? "text-red-400"
    : isStale
      ? "text-yellow-400"
      : "text-white";

  const fearLabel =
    snapshot.fearGreedIndex < 25
      ? "Extreme Fear"
      : snapshot.fearGreedIndex < 45
        ? "Fear"
        : snapshot.fearGreedIndex < 55
          ? "Neutral"
          : snapshot.fearGreedIndex < 75
            ? "Greed"
            : "Extreme Greed";

  const fearColor =
    snapshot.fearGreedIndex < 45
      ? "text-red-400"
      : snapshot.fearGreedIndex < 55
        ? "text-yellow-400"
        : "text-emerald-400";

  return (
    <Card>
      <CardHeader>
        <CardTitle>Market</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {snapshot.totalMarketCapUsd > 0 && (
          <div className="flex items-center justify-between">
            <span className="text-xs text-gray-500">Total Market Cap</span>
            <span className="text-xs font-medium text-white">
              {formatCurrency(snapshot.totalMarketCapUsd, { compact: true })}
            </span>
          </div>
        )}
        {snapshot.btcDominance > 0 && (
          <div className="flex items-center justify-between">
            <span className="text-xs text-gray-500">BTC Dominance</span>
            <span className="text-xs font-medium text-white">
              {snapshot.btcDominance.toFixed(1)}%
            </span>
          </div>
        )}
        <div className="flex items-center justify-between">
          <span
            className="text-xs text-gray-500"
            title="Crypto Fear & Greed Index — alternative.me, daily"
          >
            Fear & Greed
          </span>
          <span className={`text-xs font-semibold ${fearColor}`}>
            {snapshot.fearGreedIndex} · {fearLabel}
          </span>
        </div>

        <div className="border-t border-white/5 pt-3 space-y-2">
          {snapshot.assets.slice(0, 4).map((snapshotAsset) => {
            const live = livePrices[snapshotAsset.symbol];
            const asset = live
              ? {
                  ...snapshotAsset,
                  priceUsd: live.priceUsd,
                  change24h: live.change24h,
                }
              : snapshotAsset;
            const positive = asset.change24h >= 0;
            return (
              <div
                key={asset.symbol}
                className="flex items-center justify-between"
              >
                <span className="text-xs font-mono text-gray-400">
                  {asset.symbol}
                </span>
                <div className="flex items-center gap-2">
                  <span className={`text-xs font-medium ${priceColor}`}>
                    {formatCurrency(asset.priceUsd, { compact: true })}
                  </span>
                  <span
                    className={`text-[10px] flex items-center gap-0.5 ${changeColor(asset.change24h)}`}
                  >
                    {positive ? (
                      <TrendingUp className="w-2.5 h-2.5" />
                    ) : (
                      <TrendingDown className="w-2.5 h-2.5" />
                    )}
                    {formatPercent(asset.change24h)}
                  </span>
                </div>
              </div>
            );
          })}
        </div>

        {/* Provenance — trust surface (design system component). Source name
            comes off the live SSE tick (which FallbackProvider sets per call,
            so it flips defillama → pyth on fallback). Falling back to a
            hardcoded provider name here would lie during a fallback window. */}
        <div className="pt-2 border-t border-white/10">
          <ProvenanceLine
            source={
              newestTickAt > 0
                ? `${Object.values(livePrices)[0]?.source ?? "live"} · live tick`
                : "live feed"
            }
            freshness={new Date(effectiveCapturedAt).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          />
        </div>
      </CardContent>
    </Card>
  );
}
