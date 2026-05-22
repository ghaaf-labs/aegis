"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  CircleAlert,
  Loader2,
  RefreshCw,
  TrendingUp,
  TrendingDown,
} from "lucide-react";
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

type GatewayBalanceStatus = "idle" | "loading" | "ready" | "error";

const AGENT_TIMEOUT_MS = 90_000;
const PLAN_TIMEOUT_MS = 30_000;

async function withTimeout<T>(
  promise: Promise<T>,
  message: string,
  timeoutMs: number,
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(message)), timeoutMs);
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
  const {
    addDecision,
    gatewayBalanceError,
    gatewayBalanceStatus,
    setIsRebalancing,
    wallet,
  } = usePortfolioStore();
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
  const noPlan = isNoPlanError(error);
  const gatewayBlocked =
    gatewayBalanceStatus === "idle" ||
    gatewayBalanceStatus === "loading" ||
    gatewayBalanceStatus === "error";
  const planBlocked = !active || !wallet || gatewayBlocked;
  const commentaryBlocked = planBlocked;
  const elapsedSeconds = startedAt
    ? Math.max(0, Math.floor((now - startedAt) / 1000))
    : 0;
  const statusTitle = isPlanning
    ? "Building review plan"
    : isAnalyzing
      ? "Running strategist + critic"
      : planBlocked
        ? "Review is waiting on wallet state"
        : "Ready to review";
  const statusCopy = isPlanning
    ? "Converting target drift and wallet cash into concrete review legs."
    : isAnalyzing
      ? activityCopy(elapsedSeconds)
      : !active
        ? "Select a portfolio before building a review."
        : !wallet
          ? "Finish account setup before Aegis can read balances or route rebalance legs."
          : gatewayBalanceStatus === "error"
            ? "Circle Gateway did not return a confirmed balance. Aegis will not build a rebalance from stale or unknown cash."
            : gatewayBalanceStatus === "idle" ||
                gatewayBalanceStatus === "loading"
              ? "Waiting for Circle Gateway to confirm wallet cash before building a review."
              : active?.name
                ? `Build a deterministic review for "${active.name}" from confirmed holdings, target weights, and Circle Gateway cash. Use AI commentary only when you want extra reasoning.`
                : "Select a portfolio to review.";

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
    if (commentaryBlocked) {
      setError(blockedPlanMessage(gatewayBalanceStatus, gatewayBalanceError));
      return;
    }
    setIsAnalyzing(true);
    setStartedAt(Date.now());
    setNow(Date.now());
    setError(null);
    try {
      const result = await withTimeout(
        agentApi.analyze(active.id),
        "Agent commentary is taking longer than expected. Build the review plan directly; no funds move without approval.",
        AGENT_TIMEOUT_MS,
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
    if (planBlocked) {
      setError(blockedPlanMessage(gatewayBalanceStatus, gatewayBalanceError));
      return;
    }
    setIsPlanning(true);
    setIsRebalancing(true);
    setStartedAt(Date.now());
    setNow(Date.now());
    setError(null);
    try {
      const planned = await withTimeout(
        rebalanceApi.plan(active.id),
        "Plan creation is taking longer than expected. Try again in a moment.",
        PLAN_TIMEOUT_MS,
      );
      onClose();
      router.push(`/rebalance/${planned.rebalanceId}`);
    } catch (e) {
      setError(friendlyPlanError(e));
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
            Build the concrete review first. Optional AI commentary can explain
            the reasoning, but no trade runs until the approval screen.
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

            {planBlocked && (
              <BlockedPlanPanel
                gatewayBalanceError={gatewayBalanceError}
                gatewayBalanceStatus={gatewayBalanceStatus}
                hasPortfolio={!!active}
                hasWallet={!!wallet}
              />
            )}

            {error && <PlanErrorMessage message={error} />}

            <BrutalButton
              variant={planBlocked ? "ghost" : "pnl"}
              className="w-full"
              onClick={handleExecute}
              disabled={isBusy || planBlocked}
            >
              {isPlanning ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Building plan…
                </>
              ) : planBlocked ? (
                <>
                  <CircleAlert className="w-4 h-4 mr-2" />
                  Review unavailable
                </>
              ) : noPlan ? (
                <>
                  <RefreshCw className="w-4 h-4 mr-2" />
                  Check again
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4 mr-2" />
                  Build review plan
                </>
              )}
            </BrutalButton>
            <BrutalButton
              variant="ghost"
              className="w-full"
              onClick={handleAnalyze}
              disabled={isBusy || commentaryBlocked}
            >
              {isAnalyzing ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Getting commentary…
                </>
              ) : commentaryBlocked ? (
                <>Commentary locked until balances are trusted</>
              ) : (
                <>Add strategist commentary</>
              )}
            </BrutalButton>
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

            {planBlocked && (
              <BlockedPlanPanel
                gatewayBalanceError={gatewayBalanceError}
                gatewayBalanceStatus={gatewayBalanceStatus}
                hasPortfolio={!!active}
                hasWallet={!!wallet}
              />
            )}

            {error && <PlanErrorMessage message={error} />}

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
                variant={planBlocked ? "ghost" : "pnl"}
                className="flex-1"
                onClick={handleExecute}
                disabled={isPlanning || planBlocked}
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

function friendlyPlanError(error: unknown) {
  const raw = (error as Error).message || "Plan creation failed";
  const message = raw.replace(/^\d{3}:\s*/, "").replace(/^conflict:\s*/i, "");
  if (message.toLowerCase().includes("no rebalance plan was created")) {
    return message;
  }
  if (message.toLowerCase().includes("stale_plan")) {
    return "Holdings or wallet cash changed while this plan was loading. Build a fresh review.";
  }
  return message;
}

function blockedPlanMessage(
  gatewayBalanceStatus: GatewayBalanceStatus,
  gatewayBalanceError: string | null,
) {
  if (gatewayBalanceStatus === "error") {
    return (
      gatewayBalanceError ??
      "Circle Gateway balance is unavailable. Aegis will not build a review from unknown wallet cash."
    );
  }
  if (gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading") {
    return "Circle Gateway is still confirming wallet cash. Wait for the balance check before building a rebalance review.";
  }
  return "Finish account setup before building a rebalance review.";
}

function isNoPlanError(message: string | null) {
  return (
    message?.toLowerCase().includes("no rebalance plan was created") ?? false
  );
}

function isWalletSetupError(message: string | null) {
  return message?.toLowerCase().includes("complete account setup") ?? false;
}

function BlockedPlanPanel({
  gatewayBalanceError,
  gatewayBalanceStatus,
  hasPortfolio,
  hasWallet,
}: {
  gatewayBalanceError: string | null;
  gatewayBalanceStatus: GatewayBalanceStatus;
  hasPortfolio: boolean;
  hasWallet: boolean;
}) {
  const checks = [
    {
      label: "Portfolio",
      value: hasPortfolio ? "selected" : "missing",
      ok: hasPortfolio,
    },
    {
      label: "Wallet",
      value: hasWallet ? "connected" : "setup required",
      ok: hasWallet,
    },
    {
      label: "Gateway",
      value:
        gatewayBalanceStatus === "ready"
          ? "confirmed"
          : gatewayBalanceStatus === "error"
            ? "unavailable"
            : "checking",
      ok: gatewayBalanceStatus === "ready",
    },
  ];

  return (
    <div className="rounded-sharp border border-warn/40 bg-warn/5 p-3 font-mono">
      <div className="flex items-start gap-2">
        <CircleAlert className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-widest text-warn">
            Review plan locked
          </p>
          <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
            Aegis needs a portfolio, a completed account, and a confirmed Circle
            Gateway balance before it can calculate rebalance legs. Unknown
            wallet cash is not treated as zero.
          </p>
        </div>
      </div>
      <div className="mt-3 grid gap-2 sm:grid-cols-3">
        {checks.map((check) => (
          <div
            key={check.label}
            className={`border px-2 py-2 ${
              check.ok
                ? "border-accent-agent/30 bg-accent-agent/5"
                : "border-warn/40 bg-bg"
            }`}
          >
            <p className="text-[9px] uppercase tracking-widest text-text-mut">
              {check.label}
            </p>
            <p
              className={`mt-1 text-[11px] ${
                check.ok ? "text-accent-agent" : "text-warn"
              }`}
            >
              {check.value}
            </p>
          </div>
        ))}
      </div>
      {gatewayBalanceStatus === "error" && (
        <p className="mt-3 border border-warn/30 bg-bg px-2 py-1.5 text-[11px] leading-relaxed text-warn">
          {gatewayBalanceError ??
            "Circle Gateway did not return balances for this wallet."}
        </p>
      )}
      {!hasWallet && (
        <Link
          href="/wallets"
          className="mt-3 inline-flex min-h-8 items-center justify-center border border-warn/40 px-2 py-1 text-[11px] font-semibold text-warn hover:bg-warn/10"
        >
          Check account setup
        </Link>
      )}
      {hasWallet && gatewayBalanceStatus === "error" && (
        <Link
          href="/wallets"
          className="mt-3 inline-flex min-h-8 items-center justify-center border border-warn/40 px-2 py-1 text-[11px] font-semibold text-warn hover:bg-warn/10"
        >
          Open wallet status
        </Link>
      )}
    </div>
  );
}

function PlanErrorMessage({ message }: { message: string }) {
  const noPlan = isNoPlanError(message);
  const walletSetup = isWalletSetupError(message);
  return (
    <div
      className={`p-3 rounded-sharp border text-xs ${
        noPlan
          ? "bg-accent-agent/10 border-accent-agent/30 text-accent-agent"
          : walletSetup
            ? "bg-warn/10 border-warn/40 text-warn"
            : "bg-risk/10 border-risk/30 text-risk"
      }`}
    >
      <p>{message}</p>
      {noPlan && (
        <div className="mt-3 grid gap-2 sm:grid-cols-2">
          <Link
            href="/onboarding"
            className="inline-flex min-h-9 items-center justify-center border border-accent-agent/40 px-2 py-1 text-[11px] font-semibold text-accent-agent hover:bg-accent-agent/10"
          >
            Change target
          </Link>
          <Link
            href="/wallets"
            className="inline-flex min-h-9 items-center justify-center border border-accent-agent/40 px-2 py-1 text-[11px] font-semibold text-accent-agent hover:bg-accent-agent/10"
          >
            Add wallet cash
          </Link>
        </div>
      )}
      {walletSetup && (
        <Link
          href="/wallets"
          className="mt-2 inline-flex border border-warn/40 px-2 py-1 text-[11px] text-warn hover:bg-warn/10"
        >
          Check account setup
        </Link>
      )}
    </div>
  );
}

function activityCopy(elapsedSeconds: number) {
  if (elapsedSeconds < 8) {
    return "Reading positions, wallet cash, target weights, and recent decisions.";
  }
  if (elapsedSeconds < 20) {
    return "Waiting on the model gateway. Real market and wallet checks can take a little longer.";
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
