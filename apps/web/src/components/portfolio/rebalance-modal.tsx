"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Loader2, RefreshCw, TrendingUp, TrendingDown } from "lucide-react";
import { BrutalButton } from "@aegis/ui";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { agentApi, rebalanceApi } from "@/lib/api";
import type { AgentDecision } from "@/types";
import { formatCurrency } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

const AGENT_TIMEOUT_MS = 30_000;

async function withTimeout<T>(
  promise: Promise<T>,
  message: string,
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(message)), AGENT_TIMEOUT_MS);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    if (timeoutId) clearTimeout(timeoutId);
  }
}

export function RebalanceModal({ open, onClose }: Props) {
  const router = useRouter();
  const active = useActivePortfolio();
  const { addDecision, setIsRebalancing } = usePortfolioStore();
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [isPlanning, setIsPlanning] = useState(false);
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [now, setNow] = useState(Date.now());
  const [decision, setDecision] = useState<AgentDecision | null>(null);
  const [error, setError] = useState<string | null>(null);
  const analyzed = decision !== null;
  const recommendation = decision?.recommendation;
  const criticBlocked =
    decision?.criticVerdict?.demandsRevision === true ||
    decision?.criticVerdict?.verdict === "revised" ||
    decision?.criticVerdict?.verdict === "veto";
  const isBusy = isAnalyzing || isPlanning;
  const elapsedSeconds = startedAt
    ? Math.max(0, Math.floor((now - startedAt) / 1000))
    : 0;
  const statusTitle = isPlanning
    ? "Building review plan"
    : isAnalyzing
      ? "Running strategist + critic"
      : "Ready to analyze";
  const statusCopy = isPlanning
    ? "Converting target drift and wallet cash into concrete review legs."
    : isAnalyzing
      ? activityCopy(elapsedSeconds)
      : active?.name
        ? `Run the strategist + critic loop on "${active.name}" — it checks your target allocation, current holdings, wallet cash, and recent decisions before proposing any moves.`
        : "Select a portfolio to analyze.";

  useEffect(() => {
    if (!isBusy) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [isBusy]);

  const handleAnalyze = async () => {
    if (!active) {
      setError("Select a portfolio first.");
      return;
    }
    setIsAnalyzing(true);
    setStartedAt(Date.now());
    setNow(Date.now());
    setError(null);
    try {
      const result = await withTimeout(
        agentApi.analyze(active.id),
        "Agent analysis is taking longer than expected. Try again in a moment.",
      );
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
    setStartedAt(Date.now());
    setNow(Date.now());
    setError(null);
    try {
      const planned = await withTimeout(
        rebalanceApi.plan(active.id),
        "Plan creation is taking longer than expected. Try again in a moment.",
      );
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
      <DialogContent className="w-full sm:max-w-lg max-h-[90dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <RefreshCw className="w-4 h-4 text-accent-agent" />
            Portfolio Rebalance
          </DialogTitle>
          <DialogDescription>
            Step 1 analyzes drift. Step 2 builds a review plan. Nothing changes
            until you approve the next screen.
          </DialogDescription>
        </DialogHeader>

        {!analyzed ? (
          <div className="space-y-4">
            <div className="p-4 rounded-sharp bg-accent-agent/10 border-brutal border-accent-agent/30">
              <div className="flex items-start gap-2.5">
                {isBusy ? (
                  <Loader2 className="w-4 h-4 text-accent-agent shrink-0 mt-0.5 animate-spin" />
                ) : (
                  <RefreshCw className="w-4 h-4 text-accent-agent shrink-0 mt-0.5" />
                )}
                <div>
                  <p className="text-sm font-medium text-accent-agent mb-1">
                    {statusTitle}
                  </p>
                  <p className="text-xs text-text-lo">{statusCopy}</p>
                </div>
              </div>
            </div>

            {isBusy && (
              <ActivityProgress
                elapsedSeconds={elapsedSeconds}
                mode={isPlanning ? "plan" : "analysis"}
              />
            )}

            {error && (
              <div className="p-3 rounded-sharp bg-risk/10 border border-risk/30 text-xs text-risk">
                {error}
              </div>
            )}

            <BrutalButton
              variant="agent"
              className="w-full"
              onClick={handleAnalyze}
              disabled={isBusy || !active}
            >
              {isBusy ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  {isPlanning ? "Building plan…" : "Analyzing portfolio..."}
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4 mr-2" />
                  Run strategist + critic
                </>
              )}
            </BrutalButton>
            {error && active && !isAnalyzing && (
              <BrutalButton
                variant="ghost"
                className="w-full"
                onClick={handleExecute}
                disabled={isPlanning}
              >
                {isPlanning ? (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    Building plan…
                  </>
                ) : (
                  <>Build review plan directly</>
                )}
              </BrutalButton>
            )}
          </div>
        ) : (
          <div className="space-y-4">
            <div className="p-4 rounded-sharp bg-accent-agent/10 border-brutal border-accent-agent/30">
              {criticBlocked && (
                <p className="mb-3 rounded-sharp border border-risk/30 bg-risk/10 px-3 py-2 text-[11px] font-mono text-risk">
                  Critic blocked this analysis. Treat the text below as audit
                  history; the review plan will be rebuilt before approval.
                </p>
              )}
              <p className="text-xs text-text-lo mb-3 leading-relaxed">
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
              <p className="text-xs text-text-mut font-medium uppercase tracking-wider">
                Proposed trades
              </p>
              {recommendation?.trades?.length ? (
                recommendation.trades.map((trade, index) => {
                  const rawAction = (trade as { action?: unknown }).action;
                  const action =
                    rawAction === "buy" || rawAction === "sell"
                      ? rawAction
                      : "review";
                  return (
                    <div
                      key={`${trade.symbol ?? "unknown"}-${action}-${index}`}
                      className="flex items-center justify-between p-3 rounded-sharp bg-raised border border-border-default"
                    >
                      <div className="flex items-center gap-3">
                        <span
                          className={`px-2 py-0.5 rounded text-xs font-semibold ${
                            action === "review"
                              ? "bg-white/5 text-text-mut"
                              : action === "buy"
                                ? "bg-accent-pnl/15 text-accent-pnl"
                                : "bg-risk/15 text-risk"
                          }`}
                        >
                          {action === "buy" ? (
                            <TrendingUp className="w-3 h-3 inline mr-1" />
                          ) : action === "sell" ? (
                            <TrendingDown className="w-3 h-3 inline mr-1" />
                          ) : null}
                          {action.toUpperCase()}
                        </span>
                        <div>
                          <p className="text-sm font-semibold text-text-hi font-mono">
                            {trade.symbol ?? "UNKNOWN"}
                          </p>
                          <p className="text-[11px] text-text-mut">
                            {Number.isFinite(trade.quantity)
                              ? `${trade.quantity} units`
                              : "quantity unavailable"}
                          </p>
                        </div>
                      </div>
                      <span className="text-sm font-medium text-text-hi">
                        {Number.isFinite(trade.valueUsd)
                          ? formatCurrency(trade.valueUsd)
                          : "Review"}
                      </span>
                    </div>
                  );
                })
              ) : (
                <div className="p-3 rounded-sharp bg-raised border border-border-default text-xs text-text-lo">
                  The analysis did not produce explicit trades. You can still
                  build a review plan from the target allocation and wallet
                  balance.
                </div>
              )}
            </div>

            {error && (
              <div className="p-3 rounded-sharp bg-risk/10 border border-risk/30 text-xs text-risk">
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <BrutalButton
                variant="ghost"
                className="flex-1"
                onClick={onClose}
                disabled={isPlanning}
              >
                Cancel
              </BrutalButton>
              <BrutalButton
                variant="pnl"
                className="flex-1"
                onClick={handleExecute}
                disabled={isPlanning || !active}
              >
                {isPlanning ? (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    Building plan…
                  </>
                ) : (
                  <>
                    {criticBlocked
                      ? "Rebuild review plan"
                      : "Build review plan"}
                  </>
                )}
              </BrutalButton>
            </div>
            {isPlanning && (
              <ActivityProgress elapsedSeconds={elapsedSeconds} mode="plan" />
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function activityCopy(elapsedSeconds: number) {
  if (elapsedSeconds < 8) {
    return "Reading positions, wallet cash, target weights, and recent decisions.";
  }
  if (elapsedSeconds < 20) {
    return "Waiting on the real-mode model gateway. This can take longer than local demo mode.";
  }
  return "Still waiting on the backend. You can close this modal; no funds move from analysis alone.";
}

function ActivityProgress({
  elapsedSeconds,
  mode,
}: {
  elapsedSeconds: number;
  mode: "analysis" | "plan";
}) {
  const steps =
    mode === "analysis"
      ? ["Read portfolio", "Strategist", "Critic"]
      : ["Read balances", "Plan legs", "Open review"];
  const activeIndex = elapsedSeconds < 8 ? 0 : elapsedSeconds < 20 ? 1 : 2;
  return (
    <div className="grid grid-cols-3 gap-2 text-[10px] font-mono">
      {steps.map((step, index) => (
        <span
          key={step}
          className={`flex min-h-8 items-center justify-center border px-2 py-1 text-center rounded-sharp ${
            index <= activeIndex
              ? "border-accent-agent bg-accent-agent/10 text-accent-agent"
              : "border-border-default bg-raised text-text-mut"
          }`}
        >
          {step}
        </span>
      ))}
    </div>
  );
}
