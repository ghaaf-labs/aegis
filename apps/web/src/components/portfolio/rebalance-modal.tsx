"use client";

import { useState } from "react";
import { Loader2, RefreshCw, TrendingUp, TrendingDown, AlertTriangle } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { usePortfolioStore } from "@/stores/portfolio";
import { MOCK_AGENT_DECISIONS } from "@/lib/mock-data";
import { formatCurrency } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function RebalanceModal({ open, onClose }: Props) {
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [analyzed, setAnalyzed] = useState(false);
  const { addDecision, setIsRebalancing } = usePortfolioStore();

  const recommendation = MOCK_AGENT_DECISIONS[0]?.recommendation;

  const handleAnalyze = async () => {
    setIsAnalyzing(true);
    await new Promise((r) => setTimeout(r, 2000));
    setIsAnalyzing(false);
    setAnalyzed(true);
  };

  const handleExecute = async () => {
    setIsRebalancing(true);
    await new Promise((r) => setTimeout(r, 1500));
    if (MOCK_AGENT_DECISIONS[0]) {
      addDecision({
        ...MOCK_AGENT_DECISIONS[0],
        id: `dec_${Date.now()}`,
        triggeredBy: "user_request",
        createdAt: new Date().toISOString(),
      });
    }
    setIsRebalancing(false);
    onClose();
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
                    BTC is 18.7% above target weight. AI analysis recommended.
                  </p>
                </div>
              </div>
            </div>

            <Button
              className="w-full bg-blue-600 hover:bg-blue-500"
              onClick={handleAnalyze}
              disabled={isAnalyzing}
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
                {MOCK_AGENT_DECISIONS[0]?.reasoning}
              </p>
              <div className="flex items-center gap-3 text-xs">
                <Badge variant="success">
                  Risk delta: {recommendation?.expectedImpact.riskDelta}
                </Badge>
                <Badge variant="default">
                  Confidence: {Math.round((MOCK_AGENT_DECISIONS[0]?.confidence ?? 0) * 100)}%
                </Badge>
              </div>
            </div>

            <div className="space-y-2">
              <p className="text-xs text-gray-500 font-medium uppercase tracking-wider">
                Proposed trades
              </p>
              {recommendation?.trades.map((trade) => (
                <div
                  key={trade.assetId}
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
                      <p className="text-sm font-semibold text-white font-mono">{trade.symbol}</p>
                      <p className="text-[11px] text-gray-500">{trade.quantity} units</p>
                    </div>
                  </div>
                  <span className="text-sm font-medium text-white">
                    {formatCurrency(trade.valueUsd)}
                  </span>
                </div>
              ))}
            </div>

            <div className="flex gap-3">
              <Button
                variant="outline"
                className="flex-1 border-white/10 text-gray-300"
                onClick={onClose}
              >
                Cancel
              </Button>
              <Button
                className="flex-1 bg-blue-600 hover:bg-blue-500"
                onClick={handleExecute}
              >
                Execute Rebalance
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
