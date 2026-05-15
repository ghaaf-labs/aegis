"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import {
  Loader2,
  RefreshCw,
  TrendingUp,
  TrendingDown,
  AlertTriangle,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { agentApi, rebalanceApi, type RebalancePlanResponse } from "@/lib/api";
import type { AgentDecision } from "@/types";
import { formatCurrency } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function RebalanceModal({ open, onClose }: Props) {
  const router = useRouter();
  const active = useActivePortfolio();
  const { addDecision, setIsRebalancing } = usePortfolioStore();
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [isPlanning, setIsPlanning] = useState(false);
  const [decision, setDecision] = useState<AgentDecision | null>(null);
  const [plan, setPlan] = useState<RebalancePlanResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const analyzed = decision !== null;
  const recommendation = decision?.recommendation;

  const handleAnalyze = async () => {
    if (!active) {
      setError("Select a portfolio first.");
      return;
    }
    setIsAnalyzing(true);
    setError(null);
    try {
      const result = await agentApi.analyze(active.id);
      setDecision(result);
      addDecision(result);
    } catch (e) {
      setError((e as Error).message || "Analysis failed");
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handleExecute = async () => {
    if (!active) return;
    setIsPlanning(true);
    setIsRebalancing(true);
    setError(null);
    try {
      const planned = await rebalanceApi.plan(active.id);
      setPlan(planned);
      onClose();
      router.push(`/rebalance/${planned.rebalanceId}`);
    } catch (e) {
      setError((e as Error).message || "Plan creation failed");
      setIsRebalancing(false);
    } finally {
      setIsPlanning(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <RefreshCw className="w-4 h-4 text-blue-400" />
            Portfolio Rebalance
          </DialogTitle>
          <DialogDescription>
            The AI agent will analyze your current allocation and generate an
            optimized rebalancing plan.
          </DialogDescription>
        </DialogHeader>

        {!analyzed ? (
          <div className="space-y-4">
            <div className="p-4 rounded-xl bg-yellow-500/5 border border-yellow-500/15">
              <div className="flex items-start gap-2.5">
                <AlertTriangle className="w-4 h-4 text-yellow-400 shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm font-medium text-yellow-300 mb-1">
                    Significant drift detected
                  </p>
                  <p className="text-xs text-gray-400">
                    {active?.name
                      ? `Analyze "${active.name}" — the strategist + critic loop will propose a rebalance against your target allocation, market regime, and recent decisions.`
                      : "Select a portfolio to analyze."}
                  </p>
                </div>
              </div>
            </div>

            {error && (
              <div className="p-3 rounded-lg bg-red-500/5 border border-red-500/15 text-xs text-red-300">
                {error}
              </div>
            )}

            <Button
              className="w-full bg-blue-600 hover:bg-blue-500"
              onClick={handleAnalyze}
              disabled={isAnalyzing || !active}
            >
              {isAnalyzing ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Analyzing portfolio...
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4 mr-2" />
                  Run AI Analysis
                </>
              )}
            </Button>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="p-4 rounded-xl bg-blue-500/5 border border-blue-500/15">
              <p className="text-xs text-gray-400 mb-3 leading-relaxed">
                {decision?.reasoning}
              </p>
              <div className="flex items-center gap-3 text-xs flex-wrap">
                {recommendation?.expectedImpact && (
                  <Badge variant="success">
                    Risk delta: {recommendation.expectedImpact.riskDelta}
                  </Badge>
                )}
                <Badge variant="default">
                  Confidence: {Math.round((decision?.confidence ?? 0) * 100)}%
                </Badge>
                {decision?.modelSlug && (
                  <Badge variant="default">{decision.modelSlug}</Badge>
                )}
              </div>
            </div>

            <div className="space-y-2">
              <p className="text-xs text-gray-500 font-medium uppercase tracking-wider">
                Proposed trades
              </p>
              {recommendation?.trades.map((trade) => (
                <div
                  key={`${trade.symbol}-${trade.action}`}
                  className="flex items-center justify-between p-3 rounded-lg bg-white/3 border border-white/5"
                >
                  <div className="flex items-center gap-3">
                    <span
                      className={`px-2 py-0.5 rounded text-xs font-semibold ${
                        trade.action === "buy"
                          ? "bg-emerald-500/15 text-emerald-400"
                          : "bg-red-500/15 text-red-400"
                      }`}
                    >
                      {trade.action === "buy" ? (
                        <TrendingUp className="w-3 h-3 inline mr-1" />
                      ) : (
                        <TrendingDown className="w-3 h-3 inline mr-1" />
                      )}
                      {trade.action.toUpperCase()}
                    </span>
                    <div>
                      <p className="text-sm font-semibold text-white font-mono">
                        {trade.symbol}
                      </p>
                      <p className="text-[11px] text-gray-500">
                        {trade.quantity} units
                      </p>
                    </div>
                  </div>
                  <span className="text-sm font-medium text-white">
                    {formatCurrency(trade.valueUsd)}
                  </span>
                </div>
              ))}
            </div>

            {error && (
              <div className="p-3 rounded-lg bg-red-500/5 border border-red-500/15 text-xs text-red-300">
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <Button
                variant="outline"
                className="flex-1 border-white/10 text-gray-300"
                onClick={onClose}
                disabled={isPlanning}
              >
                Cancel
              </Button>
              <Button
                className="flex-1 bg-blue-600 hover:bg-blue-500"
                onClick={handleExecute}
                disabled={isPlanning || !active}
              >
                {isPlanning ? (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    Building plan…
                  </>
                ) : (
                  <>Review &amp; execute</>
                )}
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
