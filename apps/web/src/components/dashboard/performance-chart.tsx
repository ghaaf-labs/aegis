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
import { Activity } from "lucide-react";
import { formatCurrency } from "@/lib/utils";
import {
  BrutalCard as Card,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  BrutalCardBody as CardContent,
  ProvenanceLine,
} from "@aegis/ui";

interface PerformancePoint {
  date: string;
  value: number;
  benchmark: number;
}

export function PerformanceChart() {
  const data: PerformancePoint[] = [];
  const hasData = data.length > 0;

  if (!hasData) {
    return null;
  }

  return (
    <Card>
      <CardHeader className="flex min-h-[52px] flex-row items-center justify-between gap-3">
        <CardTitle className="flex items-center gap-2">
          <Activity className="h-3.5 w-3.5 text-accent-pnl" />
          Performance
        </CardTitle>
        <div className="flex items-center gap-4 font-mono text-xs text-text-mut">
          <span className="flex items-center gap-1.5">
            <span className="inline-block h-0.5 w-3 bg-accent-pnl" />
            Portfolio
          </span>
          <span className="flex items-center gap-1.5">
            <span className="inline-block h-0.5 w-3 bg-text-hi/50" />
            Benchmark
          </span>
        </div>
      </CardHeader>
      <CardContent>
        <div className="h-52 relative">
          {!hasData && (
            <div className="absolute inset-0 flex items-center justify-center text-center pointer-events-none">
              <div className="text-xs text-text-mut font-mono px-4">
                No history yet — completed plans appear here
              </div>
            </div>
          )}
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart
              data={data}
              margin={{ top: 4, right: 4, left: 0, bottom: 0 }}
            >
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
                      <p className="text-text-lo font-medium">{label}</p>
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
                stroke="#00FF88"
                strokeWidth={2}
                fill="#00FF88"
                fillOpacity={0.08}
                dot={false}
              />
              <Area
                type="monotone"
                dataKey="benchmark"
                name="Benchmark"
                stroke="#FFFFFF"
                strokeWidth={1.5}
                strokeDasharray="4 2"
                fill="transparent"
                dot={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {hasData && (
          <div className="pt-2 border-t border-white/10">
            <ProvenanceLine source="completed plan outcomes" freshness="live" />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
