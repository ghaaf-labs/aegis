"use client";

import { Shield } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { usePortfolioStore } from "@/stores/portfolio";

export function RiskScoreCard() {
  const portfolio = usePortfolioStore((s) => s.portfolio);
  if (!portfolio) return null;

  const score = portfolio.riskScore;
  const pct = (score / 100) * 100;

  const label = score < 30 ? "Low Risk" : score < 60 ? "Moderate" : "High Risk";
  const color = score < 30 ? "#10b981" : score < 60 ? "#f59e0b" : "#ef4444";

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Shield className="w-3.5 h-3.5" />
          Risk Score
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between mb-3">
          <span className="text-2xl font-bold text-white">{score}</span>
          <span className="text-sm font-medium" style={{ color }}>
            {label}
          </span>
        </div>
        <div className="h-2 bg-white/5 rounded-full overflow-hidden">
          <div
            className="h-full rounded-full transition-all duration-500"
            style={{ width: `${pct}%`, background: color }}
          />
        </div>
        <div className="flex justify-between mt-1.5 text-[10px] text-gray-600">
          <span>Low</span>
          <span>Medium</span>
          <span>High</span>
        </div>
        <p className="text-[11px] text-gray-500 mt-3 leading-relaxed">
          Your portfolio risk is within your target range. The AI agent will
          alert you if market conditions push this above 65.
        </p>
      </CardContent>
    </Card>
  );
}
