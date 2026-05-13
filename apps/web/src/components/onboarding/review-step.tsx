"use client";

import { CheckCircle2, Shield, TrendingUp, Zap } from "lucide-react";
import type { RiskTolerance } from "@/types";

interface Props {
  formData: {
    riskTolerance: RiskTolerance;
    investmentHorizonMonths: number;
    initialAllocations: Array<{ symbol: string; weight: number }>;
  };
}

const RISK_ICONS: Record<RiskTolerance, React.ElementType> = {
  conservative: Shield,
  moderate: TrendingUp,
  aggressive: Zap,
};

export function ReviewStep({ formData }: Props) {
  const RiskIcon = RISK_ICONS[formData.riskTolerance];
  const allocations = formData.initialAllocations.length > 0
    ? formData.initialAllocations
    : [
        { symbol: "BTC", weight: 40 },
        { symbol: "ETH", weight: 30 },
        { symbol: "SOL", weight: 15 },
        { symbol: "BNB", weight: 15 },
      ];

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-1">Ready to launch Aegis</h2>
        <p className="text-sm text-gray-400">
          Review your configuration before we activate the AI agent.
        </p>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between p-4 rounded-xl bg-white/3 border border-white/5">
          <div className="flex items-center gap-3">
            <RiskIcon className="w-4 h-4 text-blue-400" />
            <span className="text-sm text-gray-300">Risk tolerance</span>
          </div>
          <span className="text-sm font-semibold text-white capitalize">
            {formData.riskTolerance}
          </span>
        </div>

        <div className="flex items-center justify-between p-4 rounded-xl bg-white/3 border border-white/5">
          <span className="text-sm text-gray-300">Investment horizon</span>
          <span className="text-sm font-semibold text-white">
            {formData.investmentHorizonMonths >= 12
              ? `${formData.investmentHorizonMonths / 12} year${formData.investmentHorizonMonths > 12 ? "s" : ""}`
              : `${formData.investmentHorizonMonths} months`}
          </span>
        </div>

        <div className="p-4 rounded-xl bg-white/3 border border-white/5">
          <p className="text-sm text-gray-300 mb-3">Target allocation</p>
          <div className="space-y-2">
            {allocations.map((a) => (
              <div key={a.symbol} className="flex items-center gap-2">
                <span className="text-xs font-mono text-white w-12">{a.symbol}</span>
                <div className="flex-1 h-1.5 bg-white/5 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-blue-500/60 rounded-full"
                    style={{ width: `${a.weight}%` }}
                  />
                </div>
                <span className="text-xs text-gray-400 w-8 text-right">{a.weight}%</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      <div className="flex items-start gap-3 p-4 rounded-xl bg-emerald-500/5 border border-emerald-500/15">
        <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
        <p className="text-xs text-gray-400 leading-relaxed">
          Aegis AI will monitor your portfolio 24/7, alert you to significant market
          movements, and suggest rebalances when your allocations drift beyond thresholds.
          You stay in control — all trades require your approval.
        </p>
      </div>
    </div>
  );
}
