"use client";

import { TrendingUp, TrendingDown, ArrowUpDown } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent, formatNumber, changeColor } from "@/lib/utils";
import { MOCK_PRICES } from "@/lib/mock-data";

interface Props {
  showActions?: boolean;
}

export function AssetTable({ showActions = false }: Props) {
  const portfolio = usePortfolioStore((s) => s.portfolio);

  if (!portfolio) return null;

  const priceMap = Object.fromEntries(MOCK_PRICES.map((p) => [p.symbol, p]));

  return (
    <Card>
      <CardHeader>
        <CardTitle>Holdings</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        <table className="w-full">
          <thead>
            <tr className="border-b border-white/5">
              {["Asset", "Price", "24h", "Holdings", "Value", "Weight vs Target"].map((h) => (
                <th
                  key={h}
                  className="px-5 py-3 text-left text-[11px] font-medium text-gray-500 uppercase tracking-wider"
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {portfolio.allocations.map((alloc, i) => {
              const price = priceMap[alloc.symbol];
              const drift = alloc.currentWeight - alloc.targetWeight;
              const driftAbs = Math.abs(drift);

              return (
                <tr
                  key={alloc.symbol}
                  className={`border-b border-white/3 hover:bg-white/2 transition-colors ${
                    i === portfolio.allocations.length - 1 ? "border-0" : ""
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
                  <td className="px-5 py-3.5 text-sm text-white font-medium">
                    {price ? formatCurrency(price.priceUsd) : "—"}
                  </td>
                  <td className="px-5 py-3.5">
                    {price && (
                      <span className={`text-xs flex items-center gap-1 ${changeColor(price.change24h)}`}>
                        {price.change24h >= 0 ? (
                          <TrendingUp className="w-3 h-3" />
                        ) : (
                          <TrendingDown className="w-3 h-3" />
                        )}
                        {formatPercent(price.change24h)}
                      </span>
                    )}
                  </td>
                  <td className="px-5 py-3.5 text-xs text-gray-400 font-mono">
                    {formatNumber(alloc.quantity)}
                  </td>
                  <td className="px-5 py-3.5 text-sm text-white font-medium">
                    {formatCurrency(alloc.valueUsd)}
                  </td>
                  <td className="px-5 py-3.5">
                    <div className="flex items-center gap-2">
                      <div className="flex items-center gap-1">
                        <span className="text-xs text-gray-400 font-mono w-8">
                          {alloc.currentWeight.toFixed(1)}%
                        </span>
                        <span className="text-gray-600 text-xs">vs</span>
                        <span className="text-xs text-gray-500 font-mono w-8">
                          {alloc.targetWeight.toFixed(0)}%
                        </span>
                      </div>
                      {driftAbs > 3 && (
                        <Badge
                          variant={driftAbs > 10 ? "danger" : "warning"}
                          className="text-[10px] px-1.5 py-0"
                        >
                          {drift > 0 ? "+" : ""}{drift.toFixed(1)}%
                        </Badge>
                      )}
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </CardContent>
    </Card>
  );
}
