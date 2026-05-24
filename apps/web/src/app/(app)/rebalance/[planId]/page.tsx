"use client";

import { useEffect, useState, use } from "react";
import Link from "next/link";
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
  const [freshReviewLoading, setFreshReviewLoading] = useState(false);
  const [freshReviewError, setFreshReviewError] = useState<string | null>(null);
  const [decision, setDecision] = useState<AgentDecision | null>(null);
  const [approvalSafety, setApprovalSafety] =
    useState<RebalanceApprovalSafety | null>(null);
  const hasCrossChainLeg = plan?.legs.some(isCrossChainLeg) ?? false;

  useEffect(() => {
    let cancelled = false;
    setShowApproval(true);
    setApproved(false);
    setPlanStatus("planned");
    setPlan(null);
    setPortfolioId(null);
    setEstimatedFee(0);
    setFeeFetchedAt(null);
    setFeeSource("plan");
    setLoadError(null);
    setFreshReviewError(null);
    setDecision(null);
    setApprovalSafety(null);
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

  const handleFreshReview = async () => {
    if (!portfolioId || freshReviewLoading) return;
    setFreshReviewLoading(true);
    setFreshReviewError(null);
    try {
      const fresh = await rebalanceApi.plan(portfolioId);
      router.replace(`/rebalance/${fresh.rebalanceId}`);
    } catch (e) {
      const raw =
        e instanceof Error ? e.message : "Could not build a fresh review";
      setFreshReviewError(
        raw.replace(/^\d{3}:\s*/, "").replace(/^conflict:\s*/i, ""),
      );
    } finally {
      setFreshReviewLoading(false);
    }
  };

  if (loadError) {
    const missing = loadError.includes("404");
    return (
      <main className="min-h-screen bg-[#0A0A0A] px-6 py-8 text-text-hi">
        <section className="mx-auto max-w-2xl border-brutal border-border-default bg-surface p-5 shadow-brutal font-mono">
          <p className="text-[10px] uppercase tracking-widest text-warn">
            Review unavailable
          </p>
          <h1 className="mt-2 text-2xl font-semibold text-text-hi">
            {missing
              ? "This review is no longer available"
              : "Aegis could not open this review"}
          </h1>
          <p className="mt-3 text-sm leading-relaxed text-text-lo">
            {missing
              ? "The link may point to an old or deleted review. Build a fresh review from your current portfolio before approving any move."
              : "Your portfolio and wallet are unchanged. Open Portfolio to build a fresh review, or check Transactions for recent review history."}
          </p>
          <div className="mt-5 flex flex-col gap-2 sm:flex-row">
            <Link
              href="/portfolio"
              className="inline-flex min-h-10 flex-1 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-4 text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
            >
              Build fresh review
            </Link>
            <Link
              href="/transactions"
              className="inline-flex min-h-10 flex-1 items-center justify-center rounded-sharp border border-border-default bg-bg px-4 text-sm text-text-lo hover:border-border-hi hover:text-text-hi"
            >
              View history
            </Link>
          </div>
        </section>
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
          <>
            {planStatus === "failed" && (
              <section
                role="alert"
                className="border-brutal border-risk/50 bg-risk/5 p-4 font-mono"
              >
                <p className="text-[10px] uppercase tracking-widest text-risk">
                  Execution stopped
                </p>
                <h2 className="mt-2 text-lg font-semibold text-text-hi">
                  Build a fresh review from current balances
                </h2>
                <p className="mt-2 max-w-3xl text-xs leading-relaxed text-text-lo">
                  This review already failed and cannot be approved again. Some
                  earlier legs may have changed wallet balances, so the next
                  review must re-read Circle and size a new executable route.
                </p>
                <div className="mt-4 flex flex-col gap-2 sm:flex-row">
                  <button
                    type="button"
                    onClick={() => void handleFreshReview()}
                    disabled={!portfolioId || freshReviewLoading}
                    className="inline-flex min-h-11 flex-1 items-center justify-center border-brutal border-black bg-accent-pnl px-4 text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {freshReviewLoading
                      ? "Building fresh review..."
                      : "Build fresh review"}
                  </button>
                  <Link
                    href={portfolioId ? `/dashboard/${portfolioId}` : "/"}
                    className="inline-flex min-h-11 flex-1 items-center justify-center border border-border-default bg-bg px-4 text-sm text-text-lo hover:border-border-hi hover:text-text-hi"
                  >
                    Back to dashboard
                  </Link>
                </div>
                {freshReviewError && (
                  <p className="mt-3 border border-risk/40 bg-risk/5 px-3 py-2 text-xs text-risk">
                    {freshReviewError}
                  </p>
                )}
              </section>
            )}
            <ExecutionTrace
              rebalanceId={planId}
              sseUrl={SSE_URL}
              executionMode={plan?.executionMode}
              onStatusChange={setPlanStatus}
            />
          </>
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
      return "This review matches the current plan, but one selected route is not ready to move money. Change the target mix, then build a fresh executable review before approving.";
    case "SUPERSEDED":
      return "A newer rebalance review exists for this portfolio. Open the latest review or build a fresh one before approving.";
    case "STALE_PLAN":
      return "Wallet cash or portfolio holdings changed after this review was created. Build a fresh review so the amounts match current execution state.";
    case "BALANCE_UNAVAILABLE":
      return "Aegis cannot verify wallet cash right now, so real execution approval is paused until the balance check succeeds.";
    case "MOCK_OR_LEGACY_PLAN":
      return "This review was created outside the current real-execution path. Build a fresh review before approving.";
    default:
      return (
        safety.message ||
        "Approval needs changes for this review. Build a fresh review from Dashboard before executing."
      );
  }
}
