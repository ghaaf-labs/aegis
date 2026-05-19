"use client";

import { Shield } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { ProvenanceLine } from "@aegis/ui";

export function RiskScoreCard() {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

  if (!portfolio) return null;

  // risk_score = 50 is the DB default — never updated by risk_engine yet.
  // When nothing's invested, show "—" with a clarifying note instead of a
  // bright "Moderate" badge that suggests the agent assessed real risk.
  const isUninvested = portfolio.totalValueUsd < 0.5;

  if (isUninvested) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="w-3.5 h-3.5" />
            Risk Score
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-baseline justify-between mb-2">
            <span className="text-2xl font-bold text-text-mut">—</span>
            <span className="text-xs font-mono text-text-mut">
              awaiting deploy
            </span>
          </div>
          <p className="text-[11px] text-gray-500 leading-relaxed">
            Concentration, volatility and drift are computed once positions are
            live. Deploy idle cash to populate the score.
          </p>
          <div className="pt-3 border-t border-white/10">
            <ProvenanceLine
              source="risk engine · concentration + vol + drift"
              freshness="awaiting first deploy"
            />
          </div>
        </CardContent>
      </Card>
    );
  }

  const score = portfolio.riskScore;
  const pct = (score / 100) * 100;
  const label = score < 30 ? "Low Risk" : score < 60 ? "Moderate" : "High Risk";
  const color = score < 30 ? "#10b981" : score < 60 ? "#f59e0b" : "#ef4444";

  const ageMs = snapshot
    ? Date.now() - new Date(snapshot.capturedAt).getTime()
    : 0;
  const isStale = ageMs > 60_000;
  const isVeryStale = ageMs > 300_000;

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

        <div className="pt-3 border-t border-white/10">
          <ProvenanceLine
            source="current allocations + live prices"
            freshness={
              snapshot
                ? new Date(snapshot.capturedAt).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })
                : "live"
            }
          />
          {(isVeryStale || isStale) && (
            <span
              className={`text-[10px] ml-2 ${isVeryStale ? "text-red-400" : "text-yellow-400"}`}
            >
              stale
            </span>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
