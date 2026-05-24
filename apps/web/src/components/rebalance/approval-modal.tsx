"use client";

import { useState } from "react";
import { Loader2 } from "lucide-react";

import { rebalanceApi } from "@/lib/api";
import { cn } from "@/lib/utils";
import { BacktestPreview } from "@/components/rebalance/backtest-preview";
import {
  bridgedAmountUsdc,
  blockedLegCopy,
  blockedReviewMessage,
  destinationAmounts,
  isCrossChainLeg,
  normalizeRouteChain,
  routedAmountUsdc,
  sourceAmounts,
} from "@/components/rebalance/approval-modal/helpers";
import { ChangeSummary } from "@/components/rebalance/approval-modal/change-summary";
import { RebalanceRouteMap } from "@/components/rebalance/approval-modal/route-map";
import {
  GuardrailBand,
  ReviewFact,
} from "@/components/rebalance/approval-modal/guardrail-band";
import { AgentCheck } from "@/components/rebalance/approval-modal/agent-check";
import { LegList } from "@/components/rebalance/approval-modal/leg-list";
import { FeePreview } from "@/components/rebalance/approval-modal/fee-preview";
import type { ApprovalModalProps } from "@/components/rebalance/approval-modal/types";

export type { ApprovalModalProps } from "@/components/rebalance/approval-modal/types";

export function ApprovalModal({
  open,
  plan,
  portfolioId,
  estimatedFeeUsdc,
  feeFetchedAt,
  feeSource = "plan",
  portfolioName,
  decision,
  approvalSafety,
  onApproved,
  onClose,
}: ApprovalModalProps) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open || !plan) return null;

  const routedUsdc = routedAmountUsdc(plan);
  const isMockExecution = plan.executionMode === "mock";
  const hasCrossChainLeg = plan.legs.some(isCrossChainLeg);
  const destinations = destinationAmounts(plan);
  const sources = sourceAmounts(plan);
  const bridgedUsdc = bridgedAmountUsdc(plan);
  const bridgeLeg = plan.legs.find((leg) => leg.kind === "cross_chain_burn");
  const bridgeSourceChain = normalizeRouteChain(bridgeLeg?.srcChain ?? "arc");
  const bridgeTargetChain = normalizeRouteChain(bridgeLeg?.destChain ?? "base");
  const hasPositionSales = sources.length > 0;
  const positionSaleUsdc = sources.reduce(
    (acc, source) => acc + source.amountUsdc,
    0,
  );
  const destinationUsdc = destinations.reduce(
    (acc, destination) => acc + destination.amountUsdc,
    0,
  );
  const netTurnoverUsdc = Math.max(positionSaleUsdc, destinationUsdc);
  const approvalBlocked =
    approvalSafety?.approvable === false || isMockExecution;
  const approvalBlockCode = isMockExecution
    ? "HISTORICAL_TEST_REVIEW"
    : (approvalSafety?.code ?? "APPROVAL_BLOCKED");
  const approvalBlockMessage = approvalSafety
    ? blockedReviewMessage(approvalSafety)
    : isMockExecution
      ? "This review was created outside the real execution path. Build a fresh review before approving."
      : "Approval needs changes for this review. Build a fresh review before any execution.";
  const changeHeadline =
    plan.totalLegs === 0
      ? "No portfolio changes needed"
      : hasPositionSales
        ? `Rebalance $${netTurnoverUsdc.toFixed(2)} from overweight positions`
        : destinations.length > 0
          ? `Deploy $${routedUsdc.toFixed(2)} of wallet USDC`
          : `Route $${routedUsdc.toFixed(2)} USDC`;

  const handleApprove = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await rebalanceApi.execute(plan.rebalanceId);
      onApproved(plan.rebalanceId);
    } catch (e) {
      setError(e instanceof Error ? e.message : "approval failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      data-testid="approval-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
    >
      <div className="w-full sm:max-w-xl max-h-[90dvh] overflow-y-auto bg-[#141414] border-2 border-white/15 shadow-[8px_8px_0_0_#000]">
        <header className="px-6 py-4 border-b border-white/10 flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-text-hi">
              Review plan
            </h2>
            {portfolioName && (
              <p className="text-[11px] font-mono text-text-lo mt-1">
                {portfolioName}
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={submitting}
            className="text-text-lo hover:text-text-hi disabled:cursor-not-allowed disabled:opacity-50"
            aria-label="Close"
          >
            ×
          </button>
        </header>

        <div className="px-6 py-4">
          <ChangeSummary
            changeHeadline={changeHeadline}
            sources={sources}
            destinations={destinations}
            bridgedUsdc={bridgedUsdc}
            isMockExecution={isMockExecution}
            bridgeSourceChain={bridgeSourceChain}
            bridgeTargetChain={bridgeTargetChain}
          />

          <RebalanceRouteMap plan={plan} />

          <GuardrailBand
            approvalBlocked={approvalBlocked}
            approvalBlockCode={approvalBlockCode}
            approvalBlockMessage={approvalBlockMessage}
            approvalSafety={approvalSafety}
            portfolioId={portfolioId}
            totalLegs={plan.totalLegs}
            decision={decision}
          />

          {decision && <AgentCheck decision={decision} />}

          {plan.legs.some(
            (l) =>
              l.kind === "cross_chain_burn" || l.kind === "cross_chain_mint",
          ) && (
            <div className="mb-3 inline-flex items-center gap-2 rounded border border-cyan-500/40 bg-cyan-500/10 px-3 py-1 text-[11px] font-mono text-accent-agent">
              {isMockExecution
                ? "Historical test route"
                : "Real multi-step execution"}
            </div>
          )}

          <div className="mb-3 grid gap-2 text-xs font-mono sm:grid-cols-3">
            <ReviewFact
              label="Plan"
              value={`${plan.totalLegs} move${plan.totalLegs === 1 ? "" : "s"}`}
            />
            <ReviewFact
              label="Approval"
              value={approvalBlocked ? "Needs changes" : "Required"}
              tone={approvalBlocked ? "warn" : "agent"}
            />
            <ReviewFact
              label="Updates"
              value={approvalBlocked ? "Paused" : "Live after approval"}
            />
          </div>

          <p className="sr-only">
            {approvalBlocked ? (
              <>{blockedLegCopy(plan, approvalSafety)}</>
            ) : (
              <>
                The agent has planned <strong>{plan.totalLegs}</strong> leg
                {plan.totalLegs === 1 ? "" : "s"} to bring your portfolio toward
                its target.{" "}
                {isMockExecution
                  ? "This historical test review is shown for audit only. Build a fresh real-execution review before approving."
                  : hasCrossChainLeg
                    ? "One approval executes the full transfer plan; live updates show each leg as it confirms."
                    : "One approval executes the planned route; live updates show each leg as it confirms."}
              </>
            )}
          </p>

          <LegList plan={plan} />

          {plan.legs.some(
            (l) => l.srcSymbol === "EURC" || l.destSymbol === "EURC",
          ) && (
            <div className="border-brutal border-warn/40 bg-warn/10 p-3 mb-4 text-[11px] font-mono text-warn">
              EURC routes via DefiLlama spot rate while institutional Circle
              StableFX access is pending. Slippage may exceed institutional
              execution by ~3-5 bps.
            </div>
          )}

          <details className="mb-4 border border-white/10 bg-black/20">
            <summary className="cursor-pointer px-3 py-2 text-xs font-mono text-text-hi">
              Backtest preview
            </summary>
            <div className="px-3 pb-3">
              <BacktestPreview portfolioId={portfolioId ?? null} />
            </div>
          </details>

          <FeePreview
            plan={plan}
            routedUsdc={routedUsdc}
            estimatedFeeUsdc={estimatedFeeUsdc}
            feeSource={feeSource}
            feeFetchedAt={feeFetchedAt}
          />

          {error && (
            <p className="text-xs text-risk font-mono mb-3" role="alert">
              {error}
            </p>
          )}
          {submitting && (
            <p
              className="mb-3 flex items-center gap-2 border border-accent-pnl/40 bg-accent-pnl/5 px-3 py-2 font-mono text-[11px] text-accent-pnl"
              role="status"
              aria-live="polite"
            >
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Submitting approval and starting execution...
            </p>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-white/10 flex items-center justify-between gap-2">
          <a
            href="/policy"
            target="_blank"
            rel="noopener"
            className="text-xs text-text-lo hover:text-text-hi underline-offset-4 hover:underline"
          >
            Outcome &amp; refund policy
          </a>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              disabled={submitting}
              className="px-4 py-2 text-sm text-text-default hover:text-text-hi border border-white/10 disabled:cursor-not-allowed disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleApprove}
              disabled={submitting || approvalBlocked}
              className={cn(
                "inline-flex items-center gap-2 px-4 py-2 text-sm font-semibold border-2",
                approvalBlocked
                  ? "bg-warn/20 text-warn border-warn/40"
                  : "bg-emerald-500 text-black border-emerald-300 hover:bg-emerald-400", // PnL exception: approve = move funds, green per dual-accent rule
                "transition-colors",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              )}
            >
              {approvalBlocked ? (
                "Needs changes"
              ) : submitting ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  Submitting
                </>
              ) : (
                "Approve and move funds"
              )}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
