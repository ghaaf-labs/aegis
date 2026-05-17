"use client";

import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useActivePortfolio } from "@/stores/portfolio";
import { formatPercent } from "@/lib/utils";
import { ProvenanceLine } from "@aegis/ui";

const COLORS = [
  "#3b82f6",
  "#8b5cf6",
  "#06b6d4",
  "#10b981",
  "#f59e0b",
  "#ef4444",
];

interface Props {
  compact?: boolean;
}

export function AllocationChart({ compact = false }: Props) {
  const portfolio = useActivePortfolio();

  if (!portfolio) return null;

  const data = portfolio.allocations.map((a) => ({
    name: a.symbol,
    value: a.currentWeight,
    valueUsd: a.valueUsd,
  }));

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
                      fill={COLORS[index % COLORS.length]}
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

          <div className="flex-1 space-y-1.5">
            {data.map((item, i) => (
              <div
                key={item.name}
                className="flex items-center justify-between"
              >
                <div className="flex items-center gap-2">
                  <span
                    className="w-2 h-2 rounded-full shrink-0"
                    style={{ background: COLORS[i % COLORS.length] }}
                  />
                  <span className="text-xs text-gray-400 font-mono">
                    {item.name}
                  </span>
                </div>
                <span className="text-xs text-white font-medium">
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
