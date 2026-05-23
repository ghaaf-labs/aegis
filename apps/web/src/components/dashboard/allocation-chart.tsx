"use client";

import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent } from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { targetAllocationsForPortfolio } from "@/components/dashboard/target-allocations";
import {
  BrutalCard as Card,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  BrutalCardBody as CardContent,
  ProvenanceLine,
} from "@aegis/ui";

// Chart palette sourced from design-system tokens + complementary shades.
const CHART_COLORS = [
  "#00FF88", // accent-pnl
  "#FFB800", // warn
  "#FF2D7A", // risk
  "#A855F7", // violet
  "#F97316", // orange
  "#FFFFFF", // neutral cash/other
];

interface Props {
  compact?: boolean;
}

export function AllocationChart({ compact = false }: Props) {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const livePrices = usePortfolioStore((s) => s.livePrices);
  const liveSource = Object.values(livePrices)[0]?.source;

  if (!portfolio) return null;

  const allocations = targetAllocationsForPortfolio(portfolio);
  const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);

  const isUninvested = metrics.investedUsd < 0.5; // ~half a dollar of dust
  const data = isUninvested
    ? allocations.map((a) => ({
        name: a.symbol,
        value: a.targetWeight,
        valueUsd: 0,
      }))
    : metrics.positions.map((a) => ({
        name: a.symbol,
        value: a.currentWeight,
        valueUsd: a.valueUsd,
      }));

  if (data.length === 0 || data.every((d) => d.value === 0)) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Target Mix</CardTitle>
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

  if (isUninvested) {
    return (
      <Card>
        <CardHeader className="gap-3">
          <CardTitle className="min-w-0">Target Mix</CardTitle>
          <span className="shrink-0 border border-text-mut/30 px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-text-mut">
            plan only
          </span>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="border border-border-default bg-bg/70 px-3 py-2 font-mono">
            <p className="text-xs font-semibold text-text-hi">
              Target value after approval
            </p>
            <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
              Current holdings stay at {formatCurrency(0)} until you approve a
              plan.
            </p>
          </div>

          <div className="space-y-2">
            {data.map((item, i) => (
              <div key={item.name} className="grid gap-1 font-mono">
                <div className="flex items-center justify-between gap-3 text-xs">
                  <span className="min-w-0 truncate text-text-lo">
                    {item.name}
                  </span>
                  <span className="shrink-0 font-semibold text-text-hi">
                    {formatPercent(item.value, false)}
                  </span>
                </div>
                <div className="h-1.5 border border-border-default bg-bg">
                  <div
                    className="h-full"
                    style={{
                      width: `${Math.max(0, Math.min(item.value, 100))}%`,
                      backgroundColor: CHART_COLORS[i % CHART_COLORS.length],
                    }}
                  />
                </div>
              </div>
            ))}
          </div>

          <div className="border-t border-white/10 pt-2">
            <ProvenanceLine
              source="target allocation · no positions yet"
              freshness="live"
            />
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle className="flex items-center gap-2">
          <span>Allocation</span>
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className={compact ? "grid gap-4" : "grid gap-4"}>
          <div
            className={
              compact
                ? "mx-auto h-40 w-full max-w-[220px]"
                : "mx-auto h-32 w-full max-w-[150px]"
            }
          >
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie
                  data={data}
                  cx="50%"
                  cy="50%"
                  innerRadius={compact ? 45 : 48}
                  outerRadius={compact ? 70 : 75}
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
                        <p className="font-semibold text-text-hi">{d.name}</p>
                        <p className="text-text-lo">
                          {formatPercent(d.value, false)}
                        </p>
                      </div>
                    );
                  }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>

          <div className="min-w-0 space-y-1.5">
            {data.map((item, i) => (
              <div
                key={item.name}
                className="grid min-h-6 grid-cols-[minmax(0,1fr)_auto] items-center gap-2"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <span
                    className="w-2 h-2 rounded-full shrink-0"
                    style={{
                      background: CHART_COLORS[i % CHART_COLORS.length],
                    }}
                  />
                  <span className="text-xs text-text-lo font-mono truncate">
                    {item.name}
                  </span>
                </div>
                <span className="text-xs text-text-hi font-medium shrink-0">
                  {formatPercent(item.value, false)}
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="mt-4 pt-3 border-t border-white/10">
          <ProvenanceLine
            source={
              isUninvested
                ? "target allocation · no positions yet"
                : `current holdings × ${liveSource ?? "live"} prices`
            }
            freshness="live"
          />
        </div>
      </CardContent>
    </Card>
  );
}
