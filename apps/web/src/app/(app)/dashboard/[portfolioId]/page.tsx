"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { Loader2, Sparkles } from "lucide-react";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { AssetTable } from "@/components/dashboard/asset-table";
import { AgentReasoningFeed } from "@/components/agent/reasoning-feed";
import { PerformanceChart } from "@/components/dashboard/performance-chart";
import { MarketOverview } from "@/components/dashboard/market-overview";
import { TrustabilityCard } from "@/components/dashboard/trustability-card";
import {
  AssetControlTower,
  type ExecutionProgressSummary,
} from "@/components/dashboard/asset-control-tower";
import { RouteStackMatrix } from "@/components/dashboard/route-stack-matrix";
import { ApprovalModal } from "@/components/rebalance/approval-modal";
import { AllocationProposalModal } from "@/components/agent/allocation-proposal-modal";
import { DashboardSkeleton } from "../dashboard-loading";
import {
  agentApi,
  gatewayApi,
  isExecutablePlan,
  portfolioApi,
  rebalanceApi,
  userAgentApi,
  type RebalanceApprovalSafety,
  type RebalancePlanResponse,
} from "@/lib/api";
import { dismissProposal, isProposalDismissed } from "@/lib/proposal-dismissal";
import { pollDecisionReady } from "@/lib/decision-poll";
import type { AgentDecision } from "@/types";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { deriveDashboardBalanceModel } from "@/lib/dashboard-balance-model";

const stagger = { visible: { transition: { staggerChildren: 0.08 } } };
const fadeUp = {
  hidden: { opacity: 0, y: 16 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.4, ease: "easeOut" } },
};

const AGENT_PORTFOLIO_NAME = "Agent-managed portfolio";

export default function PortfolioDashboardPage() {
  const params = useParams<{ portfolioId: string }>();
  const searchParams = useSearchParams();
  const setActive = usePortfolioStore((s) => s.setActivePortfolio);

  useEffect(() => {
    if (params?.portfolioId) setActive(params.portfolioId);
  }, [params?.portfolioId, setActive]);

  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const tokenBalancesByChain = usePortfolioStore((s) => s.tokenBalancesByChain);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const wallet = usePortfolioStore((s) => s.wallet);
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);
  const activePortfolioDetailId = usePortfolioStore(
    (s) => s.activePortfolioDetailId,
  );
  const activePortfolioDetailStatus = usePortfolioStore(
    (s) => s.activePortfolioDetailStatus,
  );
  const decisionsPortfolioId = usePortfolioStore((s) => s.decisionsPortfolioId);
  const decisionsStatus = usePortfolioStore((s) => s.decisionsStatus);
  const marketSnapshotStatus = usePortfolioStore((s) => s.marketSnapshotStatus);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const gatewayBalanceUpdatedAt = usePortfolioStore(
    (s) => s.gatewayBalanceUpdatedAt,
  );
  const patchPortfolio = usePortfolioStore((s) => s.patchPortfolio);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const setGatewayBalanceStatus = usePortfolioStore(
    (s) => s.setGatewayBalanceStatus,
  );
  const activePortfolio = useActivePortfolio();
  const router = useRouter();
  const [deploying, setDeploying] = useState(false);
  const [deployError, setDeployError] = useState<string | null>(null);
  const [reviewPlan, setReviewPlan] = useState<RebalancePlanResponse | null>(
    null,
  );
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewDecision, setReviewDecision] = useState<AgentDecision | null>(
    null,
  );
  const [approvalSafety, setApprovalSafety] =
    useState<RebalanceApprovalSafety | null>(null);
  const [estimatedFeeUsdc, setEstimatedFeeUsdc] = useState(0);
  const [feeFetchedAt, setFeeFetchedAt] = useState<Date | null>(null);
  const [reviewMessage, setReviewMessage] = useState<string | null>(null);
  const [executionProgress, setExecutionProgress] =
    useState<ExecutionProgressSummary | null>(null);
  const decisions = usePortfolioStore((s) => s.decisions);
  const [proposalDecision, setProposalDecision] =
    useState<AgentDecision | null>(null);
  const [proposalOpen, setProposalOpen] = useState(false);
  const [reproposing, setReproposing] = useState(false);
  // Auto-pilot ON suppresses the Gate-1 modal entirely — proposals auto-apply
  // and the result shows in the activity feed + Transactions, no modal.
  const [autoPilotEnabled, setAutoPilotEnabled] = useState(false);
  const [designError, setDesignError] = useState(false);
  const proposalParam = searchParams?.get("proposal") ?? null;
  // Onboarding routes here with `?designing=1` rather than blocking on the slow
  // allocator call; this stable, mounted page kicks the design off once (below)
  // and opens Gate 1 when the proposal lands.
  const designing = searchParams?.get("designing") === "1";
  const searchParamsString = searchParams?.toString() ?? "";
  const [designHandoffActive, setDesignHandoffActive] = useState(false);
  const designStartedRef = useRef(false);
  const executionSyncedRef = useRef<string | null>(null);
  // Tracks a real unmount (not effect re-runs) so a slow in-flight proposal
  // still resolves into state instead of being discarded when the effect's deps
  // churn mid-call (the dashboard re-renders on every price/balance tick).
  const designMountedRef = useRef(true);
  useEffect(() => {
    designMountedRef.current = true;
    return () => {
      designMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    userAgentApi
      .autoPilot()
      .then((s) => {
        if (!cancelled) setAutoPilotEnabled(s.autoPilotEnabled);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  // Latest agent allocation proposal for this portfolio that has not been
  // applied yet. Gate 1 opens on this ahead of the deploy flow.
  const pendingProposal =
    decisions.find(
      (d) =>
        d.portfolioId === params?.portfolioId &&
        d.kind === "allocation_proposal" &&
        !d.allocationAppliedAt,
    ) ?? null;

  // Resolve a `?proposal=` deep-link (from onboarding) into a decision, since
  // the freshly created proposal may not be in the store yet. A deep-link is an
  // explicit intent to view, so it bypasses the dismissal memory (but not
  // auto-pilot, which never shows the modal).
  useEffect(() => {
    if (!proposalParam || autoPilotEnabled) return;
    if (proposalDecision?.id === proposalParam) return;
    const fromStore = decisions.find((d) => d.id === proposalParam);
    if (fromStore) {
      setProposalDecision(fromStore);
      setProposalOpen(true);
      return;
    }
    let cancelled = false;
    agentApi
      .decisionById(proposalParam)
      .then((d) => {
        if (cancelled) return;
        setProposalDecision(d);
        setProposalOpen(true);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [proposalParam, proposalDecision?.id, autoPilotEnabled, decisions]);

  // Fall back to the store's latest unapplied proposal when there is no
  // explicit deep-link. A proposal the user already dismissed (persisted by
  // decision id) does NOT re-open on refetch / SSE / remount, and with
  // auto-pilot ON no Gate-1 modal opens at all.
  useEffect(() => {
    if (proposalParam || proposalOpen || autoPilotEnabled) return;
    if (pendingProposal && !isProposalDismissed(pendingProposal.id)) {
      setProposalDecision(pendingProposal);
      setProposalOpen(true);
    }
  }, [proposalParam, proposalOpen, autoPilotEnabled, pendingProposal]);

  useEffect(() => {
    if (!portfoliosLoaded || activePortfolio) return;
    const fallback = portfolios[0]?.id;
    router.replace(fallback ? `/dashboard/${fallback}` : "/onboarding");
  }, [activePortfolio, portfolios, portfoliosLoaded, router]);

  const gatewayBalanceReady = gatewayBalanceStatus === "ready";
  const balanceModel = deriveDashboardBalanceModel({
    portfolio: activePortfolio,
    snapshot,
    wallet,
    unifiedUsdc,
    unifiedEurc,
    perChainUsdc,
    perChainEurc,
    extraTokenBalancesByChain: tokenBalancesByChain,
    gatewayBalanceStatus,
    gatewayBalanceError,
    gatewayBalanceUpdatedAt,
  });
  const hasInvestedPositions = balanceModel.hasInvestedPositions;
  const hasAgentTarget = balanceModel.hasAgentTarget;
  const deployableUsdc = balanceModel.deployableUsd;
  // Deploy/rebalance is only meaningful once the agent has designed a target.
  // Without one, "Review plan" dead-ends with "already matches target" (an empty
  // target reads as an all-cash portfolio), stranding the funds — so gate it on
  // hasAgentTarget and route untargeted accounts to design instead.
  const showDeploy =
    gatewayBalanceReady &&
    deployableUsdc > 5 &&
    hasAgentTarget &&
    !!activePortfolio;
  // Funded but no target designed yet (and nothing already in review): guide the
  // user to design an allocation (Gate 1) rather than the dead-end "Review plan".
  // A pending proposal the user hasn't dismissed auto-opens Gate 1; only when
  // there's none to act on (none yet, or it was dismissed) do we surface the
  // design CTA — so dismissing a proposal can't strand a funded account either.
  const pendingProposalActionable =
    !!pendingProposal && !isProposalDismissed(pendingProposal.id);
  const portfolioTitle = AGENT_PORTFOLIO_NAME;
  const showAssetDetails = hasAgentTarget || hasInvestedPositions;
  const activePortfolioId = activePortfolio?.id ?? null;
  const activePortfolioDetailSettled =
    activePortfolioId !== null &&
    activePortfolioDetailId === activePortfolioId &&
    isSettledStatus(activePortfolioDetailStatus);
  const decisionsSettled =
    activePortfolioId !== null &&
    decisionsPortfolioId === activePortfolioId &&
    isSettledStatus(decisionsStatus);
  const gatewayBalanceSettled =
    !wallet || isSettledStatus(gatewayBalanceStatus);
  const marketSnapshotSettled = isSettledStatus(marketSnapshotStatus);
  const dashboardLoading =
    !portfoliosLoaded ||
    !activePortfolio ||
    !activePortfolioDetailSettled ||
    !decisionsSettled ||
    !gatewayBalanceSettled ||
    !marketSnapshotSettled;

  const clearDesigningParam = useCallback(() => {
    const portfolioId = params?.portfolioId ?? activePortfolio?.id;
    if (!portfolioId || !designing) return;

    const nextParams = new URLSearchParams(searchParamsString);
    nextParams.delete("designing");
    const query = nextParams.toString();
    router.replace(`/dashboard/${portfolioId}${query ? `?${query}` : ""}`, {
      scroll: false,
    });
  }, [
    activePortfolio?.id,
    designing,
    params?.portfolioId,
    router,
    searchParamsString,
  ]);

  // Async allocation generation shared by the `?designing=1` handoff and the
  // retry / re-propose CTA: enqueue the job (the POST returns a `queued`
  // placeholder immediately — no 38–240s block), then poll until the worker
  // flips it to `ready`. The SSE `agent.decision` path also opens Gate-1 the
  // moment the ready decision lands — whichever wins; opening is idempotent.
  // Throws on failure/timeout so callers surface the retry CTA. Auto-pilot still
  // suppresses Gate 1.
  const generateAllocation = useCallback(async () => {
    if (!activePortfolio) return;
    const queued = await agentApi.proposeAllocation(activePortfolio.id);
    const ready = await pollDecisionReady(
      queued.id,
      () => designMountedRef.current,
    );
    if (!designMountedRef.current) return;
    setDesignHandoffActive(false);
    setDesignError(false);
    clearDesigningParam();
    setProposalDecision(ready);
    if (!autoPilotEnabled) setProposalOpen(true);
  }, [activePortfolio, autoPilotEnabled, clearDesigningParam]);

  useEffect(() => {
    if (!designing) {
      setDesignHandoffActive(false);
      designStartedRef.current = false;
      return;
    }

    const navigation = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
    if (navigation?.type === "reload") {
      setDesignHandoffActive(false);
      setDesignError(false);
      designStartedRef.current = false;
      clearDesigningParam();
      return;
    }

    setDesignHandoffActive(true);
  }, [clearDesigningParam, designing]);

  // `?designing=1` handoff from onboarding: generate exactly once. Ref-guarded
  // against a re-render / StrictMode double-invoke, and skipped when a proposal
  // (or an already-designed target) is present so a refresh with a stale
  // `?designing=1` never regenerates.
  useEffect(() => {
    if (!designHandoffActive || !activePortfolio) return;
    if (designStartedRef.current) return;
    if (pendingProposal || proposalDecision || hasAgentTarget) return;
    designStartedRef.current = true;
    setDesignError(false);
    void generateAllocation().catch(() => {
      if (!designMountedRef.current) return;
      setDesignError(true);
      clearDesigningParam();
    });
  }, [
    designHandoffActive,
    activePortfolio,
    pendingProposal,
    proposalDecision,
    hasAgentTarget,
    generateAllocation,
    clearDesigningParam,
  ]);

  const handleRepropose = async () => {
    if (!activePortfolio) return;
    setReproposing(true);
    setDesignError(false);
    try {
      await generateAllocation();
    } catch {
      // Surfaced via the designing banner's retry; keep the dashboard responsive.
      setDesignError(true);
    } finally {
      setReproposing(false);
    }
  };

  const handleOpenProposal = () => {
    const next = proposalDecision ?? pendingProposal;
    if (!next) return;
    setProposalDecision(next);
    setProposalOpen(true);
  };

  const syncPortfolioAndBalances = useCallback(async () => {
    if (!activePortfolioId) return;
    await Promise.all([
      portfolioApi
        .get(activePortfolioId)
        .then((portfolio) => {
          patchPortfolio(activePortfolioId, portfolio);
        })
        .catch(() => undefined),
      gatewayApi
        .balance()
        .then((balance) => {
          setUnifiedUsdc(balance.unifiedUsdc);
          setUnifiedEurc(balance.unifiedEurc);
          setPerChain(
            balance.perChain ?? {},
            balance.perChainEurc ?? {},
            undefined,
            balance.tokenBalancesByChain ?? {},
          );
          setGatewayBalanceStatus("ready");
        })
        .catch(() => {
          setGatewayBalanceStatus("error", "Wallet balance is unavailable.");
        }),
    ]);
  }, [
    activePortfolioId,
    patchPortfolio,
    setGatewayBalanceStatus,
    setPerChain,
    setUnifiedEurc,
    setUnifiedUsdc,
  ]);

  const refreshExecutionProgress = useCallback(
    async (rebalanceId: string) => {
      const detail = await rebalanceApi.get(rebalanceId);
      const next = executionProgressFromDetail(detail);
      setExecutionProgress(next);
      if (
        isTerminalExecutionStatus(next.status) &&
        executionSyncedRef.current !== next.rebalanceId
      ) {
        executionSyncedRef.current = next.rebalanceId;
        void syncPortfolioAndBalances();
      }
      return next;
    },
    [syncPortfolioAndBalances],
  );

  useEffect(() => {
    if (!activePortfolioId) {
      setExecutionProgress(null);
      return;
    }
    let cancelled = false;
    void rebalanceApi
      .history(activePortfolioId)
      .then((rows) => {
        if (cancelled) return;
        const row =
          rows.find((candidate) => candidate.status === "executing") ??
          rows.find((candidate) => isRecentTerminalExecution(candidate));
        if (!row) {
          setExecutionProgress(null);
          return;
        }
        const next = executionProgressFromHistory(row);
        setExecutionProgress(next);
        if (
          isTerminalExecutionStatus(next.status) &&
          executionSyncedRef.current !== next.rebalanceId
        ) {
          executionSyncedRef.current = next.rebalanceId;
          void syncPortfolioAndBalances();
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [activePortfolioId, syncPortfolioAndBalances]);

  useEffect(() => {
    if (
      !executionProgress ||
      isTerminalExecutionStatus(executionProgress.status)
    ) {
      return;
    }
    let cancelled = false;
    const poll = () => {
      void refreshExecutionProgress(executionProgress.rebalanceId).catch(() => {
        if (!cancelled) {
          setReviewMessage(
            "Execution is running; waiting for the next status.",
          );
        }
      });
    };
    poll();
    const timer = window.setInterval(poll, 3000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [
    executionProgress?.rebalanceId,
    executionProgress?.status,
    refreshExecutionProgress,
  ]);

  if (!activePortfolio || dashboardLoading) return <DashboardSkeleton />;

  const handleDeploy = async () => {
    if (!activePortfolio || deploying) return;
    if (
      executionProgress &&
      !isTerminalExecutionStatus(executionProgress.status)
    ) {
      setDeployError(
        `Plan ${executionProgress.rebalanceId.slice(
          0,
          8,
        )} is still executing (${executionProgress.completedLegs}/${executionProgress.totalLegs} moves confirmed). Open the trace before building another review.`,
      );
      setReviewMessage(null);
      return;
    }
    setDeploying(true);
    setDeployError(null);
    setReviewMessage("Building execution review...");
    try {
      const planned = await withTimeout(
        rebalanceApi.plan(activePortfolio.id),
        "Plan creation is taking longer than expected. Try again in a moment.",
      );
      if (!isExecutablePlan(planned)) {
        // A no-op (on-target / USDC reserve / unfunded / dust) is a calm or
        // actionable outcome, never a red error. Surface it as an agent notice.
        setExecutionProgress(null);
        setReviewPlan(null);
        setReviewOpen(false);
        setDeployError(null);
        setReviewMessage(planned.message);
        return;
      }
      setExecutionProgress(null);
      setReviewPlan(planned);
      setReviewOpen(true);
      setReviewMessage(null);
      try {
        const detail = await rebalanceApi.get(planned.rebalanceId);
        setReviewPlan(rebalancePlanFromDetail(detail));
        setApprovalSafety(detail.approvalSafety ?? null);
        setEstimatedFeeUsdc(detail.totalGasUsdc ?? 0);
        setFeeFetchedAt(new Date());
        agentApi
          .decisionById(detail.decisionId)
          .then(setReviewDecision)
          .catch(() => setReviewDecision(null));
      } catch {
        setApprovalSafety(null);
        setEstimatedFeeUsdc(0);
        setFeeFetchedAt(new Date());
      }
    } catch (e) {
      const raw =
        e instanceof Error ? e.message : "Could not build review plan";
      const cleaned = raw
        .replace(/^\d{3}:\s*/, "")
        .replace(/^conflict:\s*/i, "");
      const friendly = cleaned
        .toLowerCase()
        .includes("no rebalance plan was created")
        ? cleaned
        : /parse strategist proposal|json|JSON/i.test(raw)
          ? "Aegis could not format the plan. Try Review plan again."
          : raw;
      setDeployError(friendly);
      setReviewMessage(null);
    } finally {
      setDeploying(false);
    }
  };

  return (
    <motion.div
      initial="hidden"
      animate="visible"
      variants={stagger}
      className="mx-auto w-full max-w-[1280px] space-y-5 md:space-y-6"
    >
      {autoPilotEnabled && (
        <motion.div
          variants={fadeUp}
          role="status"
          aria-live="polite"
          className="border-brutal border-accent-agent/40 bg-accent-agent/5 p-3 md:p-4 rounded-sharp flex flex-wrap items-center justify-between gap-3"
        >
          <div className="flex items-center gap-2 min-w-0">
            <span
              className="inline-block h-2 w-2 shrink-0 rounded-sharp bg-accent-agent animate-pulse"
              aria-hidden
            />
            <p className="text-xs font-mono text-text-hi">
              <span className="font-semibold text-accent-agent">
                Auto-pilot is on.
              </span>{" "}
              The agent proposes, adopts, and executes within your guardrails —
              moves run without a manual approval step.
            </p>
          </div>
          <a
            href="/settings/agent"
            className="inline-flex min-h-11 shrink-0 items-center rounded-sharp border border-accent-agent/40 px-3 font-mono text-xs font-semibold text-accent-agent hover:bg-accent-agent/10"
          >
            Manage
          </a>
        </motion.div>
      )}

      {designHandoffActive &&
        !proposalOpen &&
        !hasAgentTarget &&
        !pendingProposal && (
          <motion.div
            variants={fadeUp}
            role="status"
            aria-live="polite"
            className="border-brutal border-accent-agent/40 bg-accent-agent/5 p-3 md:p-4 rounded-sharp flex flex-wrap items-center justify-between gap-3"
          >
            <div className="flex min-w-0 items-center gap-2">
              {!designError && (
                <Loader2
                  className="h-4 w-4 shrink-0 animate-spin text-accent-agent"
                  aria-hidden
                />
              )}
              <p className="font-mono text-xs text-text-hi">
                {designError ? (
                  <>
                    <span className="font-semibold text-risk">
                      The agent could not finish designing your allocation.
                    </span>{" "}
                    Retry to run it again.
                  </>
                ) : (
                  <>
                    <span className="font-semibold text-accent-agent">
                      The agent is designing your allocation…
                    </span>{" "}
                    This takes a few seconds — Gate 1 opens automatically when
                    it is ready.
                  </>
                )}
              </p>
            </div>
            {designError && (
              <button
                type="button"
                onClick={() => void handleRepropose()}
                disabled={reproposing}
                className="inline-flex min-h-11 shrink-0 items-center gap-1 rounded-sharp border border-accent-agent/40 bg-accent-agent/5 px-3 font-mono text-xs font-semibold text-accent-agent hover:bg-accent-agent/10 disabled:opacity-50"
              >
                <Sparkles className="h-3 w-3" />
                {reproposing ? "Retrying…" : "Retry"}
              </button>
            )}
          </motion.div>
        )}

      <motion.div
        variants={fadeUp}
        role="region"
        aria-label="Portfolio controls"
        className="grid gap-5"
      >
        <AssetControlTower
          model={balanceModel}
          executionProgress={executionProgress}
          onReviewPlan={() => void handleDeploy()}
          reviewPlanLoading={deploying}
          onDesignAllocation={() => void handleRepropose()}
          onOpenProposal={handleOpenProposal}
          designLoading={reproposing || (designHandoffActive && !designError)}
          designError={designError}
          deployError={deployError}
          reviewMessage={reviewMessage}
          proposalPending={proposalOpen || pendingProposalActionable}
        />
        <RouteStackMatrix model={balanceModel} />
      </motion.div>

      <motion.div
        variants={fadeUp}
        role="region"
        aria-label="Portfolio overview"
        className="grid grid-cols-1 items-start gap-4 xl:grid-cols-[minmax(0,0.38fr)_minmax(0,0.62fr)]"
      >
        {!showDeploy && <AllocationChart model={balanceModel} />}
        <div className={showDeploy ? "xl:col-span-2" : ""}>
          <MarketOverview />
        </div>
      </motion.div>

      <motion.div variants={fadeUp}>
        <TrustabilityCard />
      </motion.div>

      <PerformanceChart />

      <motion.div
        variants={fadeUp}
        role="region"
        aria-label="Portfolio details and decision log"
        className="grid grid-cols-1 gap-5 md:gap-6"
      >
        {showAssetDetails && (
          <AssetTable
            model={balanceModel}
            onReviewPlan={() => void handleDeploy()}
            reviewPlanDisabled={
              deploying ||
              (!!executionProgress &&
                !isTerminalExecutionStatus(executionProgress.status))
            }
            reviewPlanLoading={deploying}
          />
        )}
        <AgentReasoningFeed />
      </motion.div>

      <AllocationProposalModal
        open={proposalOpen && !autoPilotEnabled}
        portfolioId={activePortfolio.id}
        decision={proposalDecision}
        onClose={() => {
          setProposalOpen(false);
          dismissProposal(proposalDecision?.id);
        }}
        onApproved={() => {
          setProposalOpen(false);
          dismissProposal(proposalDecision?.id);
          void handleDeploy();
        }}
      />

      <ApprovalModal
        open={reviewOpen}
        plan={reviewPlan}
        portfolioId={activePortfolio.id}
        portfolioName={portfolioTitle}
        estimatedFeeUsdc={estimatedFeeUsdc}
        feeFetchedAt={feeFetchedAt}
        feeSource="plan"
        decision={reviewDecision}
        approvalSafety={approvalSafety}
        onApproved={(rebalanceId) => {
          setReviewOpen(false);
          setReviewMessage("Approved. Opening the execution trace.");
          setExecutionProgress({
            rebalanceId,
            status: "executing",
            completedLegs: 0,
            totalLegs: reviewPlan?.totalLegs ?? 0,
          });
          router.push(`/rebalance/${rebalanceId}`);
        }}
        onClose={() => setReviewOpen(false)}
      />
    </motion.div>
  );
}

const PLAN_TIMEOUT_MS = 30_000;

async function withTimeout<T>(
  promise: Promise<T>,
  message: string,
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(message)), PLAN_TIMEOUT_MS);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    if (timeoutId) clearTimeout(timeoutId);
  }
}

function isSettledStatus(status: "idle" | "loading" | "ready" | "error") {
  return status === "ready" || status === "error";
}

function isTerminalExecutionStatus(status: string) {
  return ["completed", "failed", "cancelled", "canceled", "denied"].includes(
    status.toLowerCase(),
  );
}

type RebalanceDetail = Awaited<ReturnType<typeof rebalanceApi.get>>;
type RebalanceHistoryRow = Awaited<
  ReturnType<typeof rebalanceApi.history>
>[number];

function executionProgressFromDetail(
  detail: RebalanceDetail,
): ExecutionProgressSummary {
  return {
    rebalanceId: detail.id,
    status: detail.status,
    completedLegs: detail.completedLegs,
    totalLegs: detail.totalLegs,
    failureReason: detail.failureReason,
  };
}

function executionProgressFromHistory(
  row: RebalanceHistoryRow,
): ExecutionProgressSummary {
  return {
    rebalanceId: row.id,
    status: row.status,
    completedLegs: row.completedLegs,
    totalLegs: row.totalLegs,
    failureReason: row.failureReason ?? null,
  };
}

const RECENT_TERMINAL_EXECUTION_MS = 15 * 60 * 1000;

function isRecentTerminalExecution(row: RebalanceHistoryRow) {
  if (!isTerminalExecutionStatus(row.status)) return false;
  const updatedAt = Date.parse(row.updatedAt);
  return (
    Number.isFinite(updatedAt) &&
    Date.now() - updatedAt <= RECENT_TERMINAL_EXECUTION_MS
  );
}

function rebalancePlanFromDetail(
  detail: Awaited<ReturnType<typeof rebalanceApi.get>>,
): RebalancePlanResponse {
  return {
    rebalanceId: detail.id,
    decisionId: detail.decisionId,
    executionMode: detail.executionMode,
    totalLegs: detail.totalLegs,
    legs: detail.legs.map((leg) => ({
      legIndex: leg.legIndex,
      kind: leg.kind,
      srcChain: leg.srcChain,
      destChain: leg.destChain,
      srcSymbol: leg.srcSymbol,
      destSymbol: leg.destSymbol,
      amountUsdc: leg.amountUsdc,
    })),
  };
}
