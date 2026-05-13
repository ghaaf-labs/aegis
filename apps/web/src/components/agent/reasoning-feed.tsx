"use client";

import { motion, AnimatePresence } from "framer-motion";
import { Brain, RefreshCw, ChevronRight, Zap } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { usePortfolioStore } from "@/stores/portfolio";
import { timeAgo } from "@/lib/utils";

const TRIGGER_LABELS: Record<string, string> = {
  drift_threshold: "Drift Alert",
  market_movement: "Market Signal",
  scheduled: "Scheduled",
  risk_breach: "Risk Breach",
  user_request: "Manual",
};

const TRIGGER_VARIANTS: Record<string, "warning" | "default" | "secondary" | "danger"> = {
  drift_threshold: "warning",
  market_movement: "default",
  scheduled: "secondary",
  risk_breach: "danger",
  user_request: "secondary",
};

export function AgentReasoningFeed() {
  const decisions = usePortfolioStore((s) => s.decisions);

  return (
    <Card className="flex flex-col">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <CardTitle className="flex items-center gap-2">
          <Brain className="w-3.5 h-3.5 text-blue-400" />
          AI Reasoning
        </CardTitle>
        <Button
          variant="ghost"
          size="sm"
          className="text-gray-500 hover:text-gray-300 h-7 px-2"
        >
          <RefreshCw className="w-3.5 h-3.5" />
        </Button>
      </CardHeader>
      <CardContent className="flex-1 p-0 overflow-hidden">
        <div className="overflow-y-auto max-h-[480px] scrollbar-thin">
          <AnimatePresence initial={false}>
            {decisions.map((decision, i) => (
              <motion.div
                key={decision.id}
                initial={{ opacity: 0, x: 8 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: i * 0.05 }}
                className="px-5 py-4 border-b border-white/4 last:border-0 hover:bg-white/2 transition-colors cursor-pointer group"
              >
                <div className="flex items-start justify-between gap-3 mb-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <Badge
                      variant={TRIGGER_VARIANTS[decision.triggeredBy] ?? "secondary"}
                      className="text-[10px] px-1.5 py-0"
                    >
                      {TRIGGER_LABELS[decision.triggeredBy] ?? decision.triggeredBy}
                    </Badge>
                    <span className="text-[10px] text-gray-600">
                      {timeAgo(decision.createdAt)}
                    </span>
                  </div>
                  <div className="flex items-center gap-1 shrink-0">
                    <Zap className="w-3 h-3 text-yellow-500/70" />
                    <span className="text-[10px] text-yellow-500/70 font-medium">
                      {Math.round(decision.confidence * 100)}%
                    </span>
                  </div>
                </div>

                <p className="text-xs font-semibold text-white mb-1.5">
                  {decision.recommendation.summary}
                </p>

                <p className="text-[11px] text-gray-500 leading-relaxed line-clamp-3">
                  {decision.reasoning}
                </p>

                {decision.recommendation.trades.length > 0 && (
                  <div className="mt-3 space-y-1.5">
                    {decision.recommendation.trades.map((trade) => (
                      <div
                        key={trade.assetId}
                        className="flex items-center gap-2 text-[11px]"
                      >
                        <span
                          className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                            trade.action === "buy"
                              ? "bg-emerald-500/15 text-emerald-400"
                              : "bg-red-500/15 text-red-400"
                          }`}
                        >
                          {trade.action.toUpperCase()}
                        </span>
                        <span className="font-mono text-white">{trade.symbol}</span>
                        <span className="text-gray-500">{trade.reason}</span>
                      </div>
                    ))}
                  </div>
                )}

                <button className="mt-2 flex items-center gap-1 text-[11px] text-blue-400/60 hover:text-blue-400 group-hover:opacity-100 opacity-0 transition-all">
                  View full analysis
                  <ChevronRight className="w-3 h-3" />
                </button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      </CardContent>
    </Card>
  );
}
