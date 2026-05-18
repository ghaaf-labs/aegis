"use client";

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { formatCurrency } from "@/lib/utils";
import { ProvenanceLine } from "@aegis/ui";

interface PerformancePoint {
  date: string;
  value: number;
  benchmark: number;
}

export function PerformanceChart() {
  const data: PerformancePoint[] = [];
  const hasData = data.length > 0;

  // Until a portfolio has at least one rebalance + 24h of outcome
  // history, this chart has nothing to render. Showing an empty "No
  // history yet" panel was permanently occupying a whole dashboard row
  // with zero information value — hide it instead.
  if (!hasData) {
    return null;
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <CardTitle>Performance (30d)</CardTitle>
        <div className="flex items-center gap-4 text-xs text-gray-500">
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-0.5 bg-blue-500 inline-block rounded" />
            Portfolio
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-0.5 bg-violet-500/50 inline-block rounded" />
            Benchmark
          </span>
        </div>
      </CardHeader>
      <CardContent>
        <div className="h-52 relative">
          {!hasData && (
            <div className="absolute inset-0 flex items-center justify-center text-center pointer-events-none">
              <div className="text-xs text-text-mut font-mono px-4">
                No history yet — first rebalance lands here
              </div>
            </div>
          )}
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart
              data={data}
              margin={{ top: 4, right: 4, left: 0, bottom: 0 }}
            >
              <defs>
                <linearGradient
                  id="portfolioGradient"
                  x1="0"
                  y1="0"
                  x2="0"
                  y2="1"
                >
                  <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.2} />
                  <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
                </linearGradient>
                <linearGradient
                  id="benchmarkGradient"
                  x1="0"
                  y1="0"
                  x2="0"
                  y2="1"
                >
                  <stop offset="5%" stopColor="#8b5cf6" stopOpacity={0.1} />
                  <stop offset="95%" stopColor="#8b5cf6" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid
                strokeDasharray="3 3"
                stroke="rgba(255,255,255,0.04)"
              />
              <XAxis
                dataKey="date"
                tick={{ fill: "#6b7280", fontSize: 10 }}
                axisLine={false}
                tickLine={false}
                interval={4}
              />
              <YAxis
                tick={{ fill: "#6b7280", fontSize: 10 }}
                axisLine={false}
                tickLine={false}
                tickFormatter={(v: number) =>
                  formatCurrency(v, { compact: true })
                }
                width={56}
              />
              <Tooltip
                content={({ active, payload, label }) => {
                  if (!active || !payload?.length) return null;
                  return (
                    <div className="bg-surface border-brutal border-border-default rounded-sharp p-3 text-xs space-y-1">
                      <p className="text-gray-400 font-medium">{label}</p>
                      {payload.map((p) => (
                        <p key={p.name} style={{ color: p.color }}>
                          {p.name}: {formatCurrency(Number(p.value))}
                        </p>
                      ))}
                    </div>
                  );
                }}
              />
              <Area
                type="monotone"
                dataKey="value"
                name="Portfolio"
                stroke="#3b82f6"
                strokeWidth={2}
                fill="url(#portfolioGradient)"
                dot={false}
              />
              <Area
                type="monotone"
                dataKey="benchmark"
                name="Benchmark"
                stroke="#8b5cf6"
                strokeWidth={1.5}
                strokeDasharray="4 2"
                fill="url(#benchmarkGradient)"
                dot={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {hasData && (
          <div className="pt-2 border-t border-white/10">
            <ProvenanceLine
              source="on-chain outcomes + counterfactual replay"
              freshness="live"
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
