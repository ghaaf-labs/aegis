"use client";

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Line,
  ComposedChart,
} from "recharts";

export interface Sample {
  observedAt: string;
  predictedLabel: "risk_on" | "neutral" | "risk_off" | string;
  realizedLabel: "risk_on" | "neutral" | "risk_off" | string;
}

interface Props {
  samples: Sample[];
}

const SCORE: Record<string, number> = {
  risk_off: -1,
  neutral: 0,
  risk_on: 1,
};

export function BacktestChart({ samples }: Props) {
  const data = samples.map((s) => ({
    date: new Date(s.observedAt).toISOString().slice(0, 10),
    predicted: SCORE[s.predictedLabel] ?? 0,
    realized: SCORE[s.realizedLabel] ?? 0,
  }));

  return (
    <div className="h-72">
      <ResponsiveContainer width="100%" height="100%">
        <ComposedChart
          data={data}
          margin={{ top: 8, right: 8, left: 0, bottom: 0 }}
        >
          <defs>
            <linearGradient id="predictedGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#22d3ee" stopOpacity={0.5} />
              <stop offset="100%" stopColor="#22d3ee" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid
            stroke="rgba(255,255,255,0.04)"
            strokeDasharray="3 3"
          />
          <XAxis
            dataKey="date"
            tick={{ fill: "#6b7280", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
            interval={Math.max(1, Math.floor(data.length / 12))}
          />
          <YAxis
            domain={[-1.2, 1.2]}
            ticks={[-1, 0, 1]}
            tickFormatter={(v) =>
              v === -1 ? "RISK-OFF" : v === 0 ? "NEUTRAL" : "RISK-ON"
            }
            tick={{ fill: "#6b7280", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
            width={70}
          />
          <Tooltip
            content={({ active, payload, label }) => {
              if (!active || !payload?.length) return null;
              return (
                <div className="bg-surface border-brutal border-border-default rounded-sharp p-3 text-xs font-mono space-y-1">
                  <p className="text-text-mut">{label}</p>
                  {payload.map((p) => (
                    <p key={p.name} style={{ color: p.color }}>
                      {p.name}:{" "}
                      {p.value === 1
                        ? "RISK-ON"
                        : p.value === 0
                          ? "NEUTRAL"
                          : "RISK-OFF"}
                    </p>
                  ))}
                </div>
              );
            }}
          />
          <Area
            type="stepAfter"
            dataKey="predicted"
            name="predicted"
            stroke="#22d3ee"
            strokeWidth={1.5}
            fill="url(#predictedGradient)"
            dot={false}
          />
          <Line
            type="stepAfter"
            dataKey="realized"
            name="realized"
            stroke="#9ca3af"
            strokeDasharray="4 3"
            strokeWidth={1}
            dot={false}
          />
        </ComposedChart>
      </ResponsiveContainer>
    </div>
  );
}
