"use client";

import { TrendingUp, TrendingDown, Wallet } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";
import { ProvenanceLine, Skeleton } from "@aegis/ui";

export function PortfolioSummaryCard() {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

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
    ? "text-red-400"
    : isStale
      ? "text-yellow-400"
      : "text-white";

  const isPositive = portfolio.totalPnlUsd >= 0;
  const TrendIcon = isPositive ? TrendingUp : TrendingDown;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5" />
          Total Value
        </CardTitle>
      </CardHeader>
      <CardContent>
        <p className={`text-3xl font-bold mb-1 ${priceColor}`}>
          {formatCurrency(portfolio.totalValueUsd)}
        </p>
        <div
          className={`flex items-center gap-1.5 text-sm ${changeColor(portfolio.totalPnlUsd)}`}
        >
          <TrendIcon className="w-3.5 h-3.5" />
          <span className="font-medium">
            {formatCurrency(portfolio.totalPnlUsd, { compact: true })}
          </span>
          <span className="text-gray-500">·</span>
          <span>{formatPercent(portfolio.totalPnlPct)}</span>
          <span className="text-gray-500 text-xs ml-1">all time</span>
        </div>

        <div className="mt-4 grid grid-cols-2 gap-3">
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-xs text-gray-500 mb-1">Assets</p>
            <p className="text-sm font-semibold text-white">
              {portfolio.allocations.length}
            </p>
          </div>
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-xs text-gray-500 mb-1">Risk Score</p>
            <div className="flex items-center gap-2">
              <p
                className={`text-sm font-semibold ${portfolio.riskScore < 40 ? "text-emerald-400" : portfolio.riskScore < 65 ? "text-yellow-400" : "text-red-400"}`}
              >
                {portfolio.riskScore}/100
              </p>
              {isVeryStale && (
                <span className="text-red-400 text-[10px]">stale</span>
              )}
              {isStale && !isVeryStale && (
                <span className="text-yellow-400 text-[10px]">stale</span>
              )}
            </div>
            {snapshot && (
              <p className="text-[10px] text-gray-500 mt-0.5">
                as of{" "}
                {new Date(snapshot.capturedAt).toLocaleTimeString([], {
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </p>
            )}
          </div>

          <div className="col-span-2 pt-2 border-t border-white/10">
            <ProvenanceLine
              source="Gateway unified balance + on-chain positions"
              freshness="live"
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
