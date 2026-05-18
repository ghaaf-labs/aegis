"use client";

import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useActivePortfolio } from "@/stores/portfolio";
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

  if (!portfolio) return null;

  const data = (portfolio.allocations ?? []).map((a) => ({
    name: a.symbol,
    value: a.currentWeight,
    valueUsd: a.valueUsd,
  }));

  const isEmpty = data.length === 0 || data.every((d) => d.value === 0);

  if (isEmpty) {
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
      <CardHeader>
        <CardTitle>Allocation</CardTitle>
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
              source="current portfolio allocations"
              freshness="live"
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
