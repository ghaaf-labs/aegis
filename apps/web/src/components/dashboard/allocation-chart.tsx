"use client";

import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { PieChart as PieChartIcon } from "lucide-react";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent } from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { targetAllocationsForPortfolio } from "@/components/dashboard/target-allocations";
import type { DashboardBalanceModel } from "@/lib/dashboard-balance-model";
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
  model?: DashboardBalanceModel;
}

export function AllocationChart({ compact = false, model }: Props) {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const livePrices = usePortfolioStore((s) => s.livePrices);
  const liveSource = Object.values(livePrices)[0]?.source;

  if (!portfolio) return null;

  const allocations = targetAllocationsForPortfolio(portfolio);
  const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const modelData =
    model?.tokens
      .filter((token) => token.totalUsd > 0.005)
      .map((token) => ({
        name: token.symbol,
        value: token.weightPct,
        valueUsd: token.totalUsd,
      })) ?? [];

  const usingModelData = modelData.length > 0;
  const isUninvested = !usingModelData && metrics.investedUsd < 0.5;
  const data = usingModelData
    ? modelData
    : isUninvested
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
      <Card className="flex h-full min-h-[280px] flex-col">
        <CardHeader className="min-h-[52px] shrink-0">
          <CardTitle className="flex items-center gap-2">
            <PieChartIcon className="h-3.5 w-3.5 text-accent-agent" />
            Target Mix
          </CardTitle>
        </CardHeader>
        <CardContent className="flex min-h-[160px] flex-1 items-center justify-center">
          <div className="text-center">
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
      <Card className="flex h-full min-h-[280px] flex-col">
        <CardHeader className="min-h-[52px] shrink-0 gap-3">
          <CardTitle className="flex min-w-0 items-center gap-2">
            <PieChartIcon className="h-3.5 w-3.5 text-accent-agent" />
            Target Mix
          </CardTitle>
          <span className="shrink-0 border border-text-mut/30 px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-text-mut">
            plan only
          </span>
        </CardHeader>
        <CardContent className="flex flex-1 flex-col gap-3">
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
                  <span className="shrink-0 font-mono font-semibold tabular-nums text-text-hi">
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

          <div className="mt-auto border-t border-white/10 pt-2">
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
    <Card className="flex h-full min-h-[280px] flex-col">
      <CardHeader className="flex min-h-[52px] shrink-0 flex-row items-center justify-between">
        <CardTitle className="flex items-center gap-2">
          <PieChartIcon className="h-3.5 w-3.5 text-accent-agent" />
          <span>Allocation</span>
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col">
        <div className={compact ? "grid gap-4" : "grid gap-4"}>
          <div
            className={
              compact
                ? "mx-auto h-40 w-full max-w-[220px]"
                : "mx-auto h-36 w-full max-w-[170px]"
            }
          >
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie
                  data={data}
                  cx="50%"
                  cy="50%"
                  innerRadius={compact ? 45 : 40}
                  outerRadius={compact ? 70 : 66}
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
                      <div className="rounded-sharp border-brutal border-border-default bg-surface px-3 py-2 font-mono text-xs">
                        <p className="font-semibold text-text-hi">{d.name}</p>
                        <p className="tabular-nums text-text-lo">
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
                    className="w-2 h-2 rounded-sharp shrink-0"
                    style={{
                      background: CHART_COLORS[i % CHART_COLORS.length],
                    }}
                  />
                  <span className="text-xs text-text-lo font-mono truncate">
                    {item.name}
                  </span>
                </div>
                <span className="shrink-0 font-mono text-xs font-medium tabular-nums text-text-hi">
                  {formatPercent(item.value, false)}
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="mt-auto border-t border-white/10 pt-3">
          <ProvenanceLine
            source={
              isUninvested
                ? "target allocation · no positions yet"
                : usingModelData
                  ? "Circle balances + execution ledger"
                  : `current holdings × ${liveSource ?? "live"} prices`
            }
            freshness="live"
          />
        </div>
      </CardContent>
    </Card>
  );
}
