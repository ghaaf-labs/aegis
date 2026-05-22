"use client";

import { useEffect, useState, use } from "react";
import { useRouter } from "next/navigation";

import {
  rebalanceApi,
  ratesApi,
  agentApi,
  type RebalanceApprovalSafety,
  type RebalancePlanResponse,
} from "@/lib/api";
import type { AgentDecision } from "@/types";
import { ModelBadge } from "@aegis/ui";
import { ApprovalModal } from "@/components/rebalance/approval-modal";
import { ExecutionTrace } from "@/components/rebalance/execution-trace";

const SSE_URL = process.env.NEXT_PUBLIC_SSE_URL ?? "http://localhost:8080/sse";

interface PageProps {
  params: Promise<{ planId: string }>;
}

/**
 * Rebalance review + execution page. The portfolio dashboard navigates here
 * after `rebalanceApi.plan()` succeeds; this page renders the approval modal
 * first, then transitions to the realtime execution trace once the user
 * approves.
 */
export default function RebalancePage({ params }: PageProps) {
  const { planId } = use(params);
  const router = useRouter();

  const [showApproval, setShowApproval] = useState(true);
  const [estimatedFee, setEstimatedFee] = useState(0);
  const [feeFetchedAt, setFeeFetchedAt] = useState<Date | null>(null);
  const [feeSource, setFeeSource] = useState<"plan" | "paymaster">("plan");
  const [portfolioId, setPortfolioId] = useState<string | null>(null);
  const [plan, setPlan] = useState<RebalancePlanResponse | null>(null);
  const [approved, setApproved] = useState(false);
  const [planStatus, setPlanStatus] = useState<string>("planned");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [decision, setDecision] = useState<AgentDecision | null>(null);
  const [approvalSafety, setApprovalSafety] =
    useState<RebalanceApprovalSafety | null>(null);
  const hasCrossChainLeg = plan?.legs.some(isCrossChainLeg) ?? false;

  useEffect(() => {
    let cancelled = false;
    void rebalanceApi
      .get(planId)
      .then((detail) => {
        if (cancelled) return;
        setPortfolioId(detail.portfolioId);
        // Pre-approval: render the approval modal.
        setPlan({
          rebalanceId: detail.id,
          decisionId: detail.decisionId,
          executionMode: detail.executionMode,
          totalLegs: detail.totalLegs,
          legs: detail.legs.map((l) => ({
            legIndex: l.legIndex,
            kind: l.kind,
            srcChain: l.srcChain,
            destChain: l.destChain,
            srcSymbol: l.srcSymbol,
            destSymbol: l.destSymbol,
            amountUsdc: l.amountUsdc,
          })),
        });
        setEstimatedFee(detail.totalGasUsdc ?? 0);
        setFeeFetchedAt(new Date());
        setFeeSource("plan");
        setPlanStatus(detail.status);
        setApprovalSafety(detail.approvalSafety);
        // If the plan is already past 'planned' state, skip approval modal.
        if (detail.status !== "planned") {
          setShowApproval(false);
          setApproved(true);
        }
        // Fetch the AgentDecision behind this plan so the approval modal
        // can surface model_slug + critic verdict + confidence.
        agentApi
          .decisionById(detail.decisionId)
          .then((d) => {
            if (!cancelled) setDecision(d);
          })
          .catch(() => {
            // Best-effort. The modal still renders without the agentic-signal
            // header — judges then see plan-only, but execution still works.
          });
        // Refresh from Paymaster live so the user sees the current quote, not
        // the stale planner-time estimate. The destination chain of the first
        // cross-chain leg drives the lookup; default to arc.
        const firstCrossChain = detail.legs.find(
          (l) => l.kind === "cross_chain_burn" || l.kind === "cross_chain_mint",
        );
        const chain: "arc" | "base" =
          (firstCrossChain?.destChain as "arc" | "base" | undefined) ?? "arc";
        ratesApi
          .paymasterEstimate(chain, "rebalance")
          .then((quote) => {
            if (cancelled) return;
            setEstimatedFee(quote.feeUsdc);
            setFeeFetchedAt(new Date());
            setFeeSource("paymaster");
          })
          .catch(() => {
            // Plan-time estimate is still shown; the provenance line says so.
          });
      })
      .catch((e) =>
        setLoadError(e instanceof Error ? e.message : "load failed"),
      );
    return () => {
      cancelled = true;
    };
  }, [planId]);

  if (loadError) {
    return (
      <main className="p-8 text-risk font-mono text-sm">
        Failed to load rebalance plan: {loadError}
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-[#0A0A0A] text-text-hi px-6 py-8">
      <div className="max-w-5xl mx-auto space-y-6">
        <header>
          <p className="font-mono text-[11px] tracking-wider text-accent-agent uppercase">
            Rebalance · {planId.slice(0, 8)}…
          </p>
          <h1 className="text-2xl font-bold mt-1">
            {(() => {
              if (!approved) return "Review the plan";
              if (planStatus === "completed") return "Execution complete";
              if (planStatus === "failed") return "Execution halted";
              return "Execution in progress";
            })()}
          </h1>

          {plan &&
            plan.legs.some(
              (l) =>
                l.kind === "cross_chain_burn" || l.kind === "cross_chain_mint",
            ) && (
              <div className="mt-2 inline-flex items-center gap-2 rounded border border-cyan-500/40 bg-cyan-500/10 px-3 py-1 text-[11px] font-mono text-accent-agent">
                {plan.executionMode === "mock"
                  ? "Local demo execution • simulates CCTP V2 + Hooks"
                  : "Real on-chain execution • CCTP V2 Fast Transfer + Hooks"}
              </div>
            )}

          {decision && (
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm">
              {decision.modelSlug && <ModelBadge model={decision.modelSlug} />}
              {decision.regime && (
                <span className="text-xs text-violet-300 font-mono">
                  {decision.regime.replace("_", " ")}
                </span>
              )}
              {decision.criticVerdict && (
                <span
                  className={`text-xs px-2 py-0.5 font-mono border ${
                    decision.criticVerdict.verdict === "revised" ||
                    decision.criticVerdict.demandsRevision
                      ? "border-rose-500/40 text-risk bg-rose-500/10"
                      : "border-cyan-500/40 text-accent-agent bg-cyan-500/10"
                  }`}
                >
                  Critic:{" "}
                  {decision.criticVerdict.verdict ??
                    (decision.criticVerdict.demandsRevision
                      ? "revised"
                      : "approved")}
                </span>
              )}
              <span className="text-xs text-text-mut">
                confidence {(decision.confidence * 100).toFixed(0)}%
              </span>
            </div>
          )}
        </header>

        {approved ? (
          <ExecutionTrace
            rebalanceId={planId}
            sseUrl={SSE_URL}
            executionMode={plan?.executionMode}
          />
        ) : (
          <p className="text-sm text-text-lo">
            {approvalSafety?.approvable === false
              ? blockedReviewCopy(approvalSafety)
              : plan?.executionMode === "mock"
                ? "This is a historical test review. Build a fresh real-execution review before approving."
                : hasCrossChainLeg
                  ? "The agent has built a multi-step transfer plan. Review every leg, then approve once to execute it."
                  : "The agent has built a one-network plan. Review the legs, then approve to execute the non-USDC target sleeves while the USDC sleeve stays in wallet cash."}
          </p>
        )}

        <ApprovalModal
          open={showApproval && plan !== null}
          plan={plan}
          portfolioId={portfolioId}
          estimatedFeeUsdc={estimatedFee}
          feeFetchedAt={feeFetchedAt}
          feeSource={feeSource}
          decision={decision}
          approvalSafety={approvalSafety}
          onApproved={() => {
            setShowApproval(false);
            setApproved(true);
          }}
          onClose={() => {
            setShowApproval(false);
            router.back();
          }}
        />
      </div>
    </main>
  );
}

function isCrossChainLeg(leg: { kind: string }) {
  return leg.kind === "cross_chain_burn" || leg.kind === "cross_chain_mint";
}

function blockedReviewCopy(safety: RebalanceApprovalSafety) {
  switch (safety.code) {
    case "EXECUTION_UNAVAILABLE":
      return "This review matches the current plan, but the running API cannot execute every leg in real mode. Approval stays locked until the missing adapter or cargo feature is enabled, or the target excludes that sleeve.";
    case "SUPERSEDED":
      return "A newer rebalance review exists for this portfolio. Open the latest review or build a fresh one before approving.";
    case "STALE_PLAN":
      return "Wallet cash or portfolio holdings changed after this review was created. Build a fresh review so the amounts match current execution state.";
    case "BALANCE_UNAVAILABLE":
      return "Aegis cannot verify wallet cash right now, so real execution approval is locked until the balance check succeeds.";
    case "MOCK_OR_LEGACY_PLAN":
      return "This review was created outside the current real-execution path. Build a fresh review before approving.";
    default:
      return (
        safety.message ||
        "Approval is blocked for this review. Build a fresh review from Dashboard before executing."
      );
  }
}
