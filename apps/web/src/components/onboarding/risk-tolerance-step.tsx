"use client";

import { Shield, TrendingUp, Zap } from "lucide-react";
import type { RiskTolerance } from "@/types";
import { cn } from "@/lib/utils";

const OPTIONS: Array<{
  value: RiskTolerance;
  label: string;
  description: string;
  icon: React.ElementType;
  color: string;
}> = [
  {
    value: "conservative",
    label: "Conservative",
    description: "Capital preservation. Lower volatility, slower growth. Minimal crypto exposure.",
    icon: Shield,
    color: "text-emerald-400",
  },
  {
    value: "moderate",
    label: "Moderate",
    description: "Balanced risk-reward. Mix of stable and growth assets. Recommended for most.",
    icon: TrendingUp,
    color: "text-blue-400",
  },
  {
    value: "aggressive",
    label: "Aggressive",
    description: "Maximum growth potential. High volatility accepted. Strong stomach required.",
    icon: Zap,
    color: "text-violet-400",
  },
];

interface Props {
  value: RiskTolerance;
  horizonMonths: number;
  onChange: (v: { riskTolerance?: RiskTolerance; investmentHorizonMonths?: number }) => void;
}

export function RiskToleranceStep({ value, horizonMonths, onChange }: Props) {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-1">What&apos;s your risk tolerance?</h2>
        <p className="text-sm text-gray-400">
          This shapes how Aegis rebalances your portfolio and evaluates trade-offs.
        </p>
      </div>

      <div className="space-y-3">
        {OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() => onChange({ riskTolerance: opt.value })}
            className={cn(
              "w-full flex items-start gap-4 p-4 rounded-xl border transition-all text-left",
              value === opt.value
                ? "border-blue-500/50 bg-blue-500/8"
                : "border-white/8 bg-white/2 hover:bg-white/4"
            )}
          >
            <div className={`mt-0.5 ${opt.color}`}>
              <opt.icon className="w-5 h-5" />
            </div>
            <div>
              <p className="text-sm font-semibold text-white mb-0.5">{opt.label}</p>
              <p className="text-xs text-gray-400 leading-relaxed">{opt.description}</p>
            </div>
            <div className="ml-auto shrink-0 mt-1">
              <div
                className={cn(
                  "w-4 h-4 rounded-full border-2 transition-all",
                  value === opt.value
                    ? "border-blue-500 bg-blue-500"
                    : "border-gray-600"
                )}
              />
            </div>
          </button>
        ))}
      </div>

      <div>
        <label className="text-sm font-medium text-gray-300 block mb-3">
          Investment horizon
        </label>
        <div className="grid grid-cols-4 gap-2">
          {[3, 6, 12, 24].map((months) => (
            <button
              key={months}
              onClick={() => onChange({ investmentHorizonMonths: months })}
              className={cn(
                "py-2 rounded-lg text-sm font-medium border transition-all",
                horizonMonths === months
                  ? "border-blue-500/50 bg-blue-500/10 text-blue-400"
                  : "border-white/8 bg-white/2 text-gray-400 hover:bg-white/5"
              )}
            >
              {months < 12 ? `${months}mo` : `${months / 12}yr`}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
