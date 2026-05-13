"use client";

import { TrendingUp, TrendingDown, Wallet } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";

export function PortfolioSummaryCard() {
  const portfolio = usePortfolioStore((s) => s.portfolio);

  if (!portfolio) {
    return (
      <Card className="col-span-1">
        <CardContent className="p-5">
          <div className="h-24 shimmer rounded-lg" />
        </CardContent>
      </Card>
    );
  }

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
        <p className="text-3xl font-bold text-white mb-1">
          {formatCurrency(portfolio.totalValueUsd)}
        </p>
        <div className={`flex items-center gap-1.5 text-sm ${changeColor(portfolio.totalPnlUsd)}`}>
          <TrendIcon className="w-3.5 h-3.5" />
          <span className="font-medium">
            {formatCurrency(portfolio.totalPnlUsd, { compact: true })}
          </span>
          <span className="text-gray-500">·</span>
          <span>{formatPercent(portfolio.totalPnlPct)}</span>
          <span className="text-gray-500 text-xs ml-1">all time</span>
        </div>

        <div className="mt-4 grid grid-cols-2 gap-3">
          <div className="p-3 rounded-lg bg-white/3 border border-white/5">
            <p className="text-xs text-gray-500 mb-1">Assets</p>
            <p className="text-sm font-semibold text-white">
              {portfolio.allocations.length}
            </p>
          </div>
          <div className="p-3 rounded-lg bg-white/3 border border-white/5">
            <p className="text-xs text-gray-500 mb-1">Risk Score</p>
            <p className={`text-sm font-semibold ${portfolio.riskScore < 40 ? "text-emerald-400" : portfolio.riskScore < 65 ? "text-yellow-400" : "text-red-400"}`}>
              {portfolio.riskScore}/100
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
