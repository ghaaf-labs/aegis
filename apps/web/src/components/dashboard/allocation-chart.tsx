"use client";

import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { formatPercent } from "@/lib/utils";
import { ProvenanceLine } from "@aegis/ui";

// Chart palette sourced from design-system tokens + complementary shades.
const CHART_COLORS = [
  "#00E0FF", // accent-agent
  "#FFB800", // warn
  "#FF2D7A", // risk
  "#00FF88", // accent-pnl
  "#A855F7", // violet
  "#F97316", // orange
];

interface Props {
  compact?: boolean;
}

export function AllocationChart({ compact = false }: Props) {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

  if (!portfolio) return null;

  const priceMap = snapshot
    ? Object.fromEntries(snapshot.assets.map((a) => [a.symbol, a.priceUsd]))
    : {};

  // Re-derive current weight from holdings × price each render — the stored
  // `current_weight` column is initialised to the target and isn't kept fresh
  // by the executor, so reading it would paint a fictional pie.
  const allocations = portfolio.allocations ?? [];
  const investedUsdBySymbol = allocations.map((a) => ({
    symbol: a.symbol,
    target: a.targetWeight,
    valueUsd: (priceMap[a.symbol] ?? 0) * a.quantity,
  }));
  const totalInvestedUsd = investedUsdBySymbol.reduce(
    (sum, a) => sum + a.valueUsd,
    0,
  );

  const isUninvested = totalInvestedUsd < 0.5; // ~half a dollar of dust
  const data = isUninvested
    ? allocations.map((a) => ({
        name: a.symbol,
        value: a.targetWeight,
        valueUsd: 0,
      }))
    : investedUsdBySymbol.map((a) => ({
        name: a.symbol,
        value: (a.valueUsd / totalInvestedUsd) * 100,
        valueUsd: a.valueUsd,
      }));

  if (data.length === 0 || data.every((d) => d.value === 0)) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Allocation</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-center h-32 text-center">
            <p className="text-xs text-text-mut font-mono px-4">
              No allocations yet — adopt a strategy or deposit USDC to get
              started
            </p>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>
          Allocation
          {isUninvested && (
            <span className="ml-2 text-[10px] font-mono text-text-mut uppercase tracking-wider">
              · target
            </span>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div
          className={`flex ${compact ? "flex-col gap-4" : "items-center gap-6"}`}
        >
          <div className={compact ? "h-40" : "h-32 w-32 shrink-0"}>
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie
                  data={data}
                  cx="50%"
                  cy="50%"
                  innerRadius={compact ? 45 : 30}
                  outerRadius={compact ? 70 : 50}
                  paddingAngle={2}
                  dataKey="value"
                  strokeWidth={0}
                >
                  {data.map((_, index) => (
                    <Cell
                      key={`cell-${index}`}
                      fill={CHART_COLORS[index % CHART_COLORS.length]}
                      opacity={0.9}
                    />
                  ))}
                </Pie>
                <Tooltip
                  content={({ active, payload }) => {
                    if (!active || !payload?.[0]) return null;
                    const d = payload[0].payload as (typeof data)[0];
                    return (
                      <div className="bg-surface border-brutal border-border-default rounded-sharp px-3 py-2 text-xs">
                        <p className="font-semibold text-white">{d.name}</p>
                        <p className="text-gray-400">
                          {formatPercent(d.value, false)}
                        </p>
                      </div>
                    );
                  }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>

          <div className="flex-1 min-w-0 space-y-1.5">
            {data.map((item, i) => (
              <div
                key={item.name}
                className="flex items-center justify-between gap-2"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <span
                    className="w-2 h-2 rounded-full shrink-0"
                    style={{
                      background: CHART_COLORS[i % CHART_COLORS.length],
                    }}
                  />
                  <span className="text-xs text-gray-400 font-mono truncate">
                    {item.name}
                  </span>
                </div>
                <span className="text-xs text-white font-medium shrink-0">
                  {formatPercent(item.value, false)}
                </span>
              </div>
            ))}
          </div>

          <div className="pt-2 border-t border-white/10">
            <ProvenanceLine
              source={
                isUninvested
                  ? "target allocation · no positions yet"
                  : "current holdings × DefiLlama prices"
              }
              freshness="live"
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
