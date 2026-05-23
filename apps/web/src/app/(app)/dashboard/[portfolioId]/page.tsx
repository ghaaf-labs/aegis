"use client";

import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useParams, useRouter } from "next/navigation";
import { CircleAlert, Loader2, LockKeyhole, Rocket } from "lucide-react";
import { PortfolioSummaryCard } from "@/components/dashboard/portfolio-summary-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { AssetTable } from "@/components/dashboard/asset-table";
import { AgentReasoningFeed } from "@/components/agent/reasoning-feed";
import { PerformanceChart } from "@/components/dashboard/performance-chart";
import { MarketOverview } from "@/components/dashboard/market-overview";
import { TrustabilityCard } from "@/components/dashboard/trustability-card";
import { IdleCashCard } from "@/components/dashboard/idle-cash-card";
import { targetAllocationsForPortfolio } from "@/components/dashboard/target-allocations";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { ApprovalModal } from "@/components/rebalance/approval-modal";
import { BrutalButton } from "@aegis/ui";
import {
  agentApi,
  rebalanceApi,
  type RebalanceApprovalSafety,
  type RebalancePlanResponse,
} from "@/lib/api";
import type { AgentDecision } from "@/types";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";

const stagger = { visible: { transition: { staggerChildren: 0.08 } } };
const fadeUp = {
  hidden: { opacity: 0, y: 16 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.4, ease: "easeOut" } },
};

export default function PortfolioDashboardPage() {
  const params = useParams<{ portfolioId: string }>();
  const setActive = usePortfolioStore((s) => s.setActivePortfolio);

  useEffect(() => {
    if (params?.portfolioId) setActive(params.portfolioId);
  }, [params?.portfolioId, setActive]);

  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const wallet = usePortfolioStore((s) => s.wallet);
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
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

  useEffect(() => {
    if (!portfoliosLoaded || activePortfolio) return;
    const fallback = portfolios[0]?.id;
    router.replace(fallback ? `/dashboard/${fallback}` : "/onboarding");
  }, [activePortfolio, portfolios, portfoliosLoaded, router]);

  const deployableUsdc = unifiedUsdc;
  const gatewayBalanceReady = gatewayBalanceStatus === "ready";
  const gatewayBalanceUnavailable = gatewayBalanceStatus === "error";
  const positionMetrics = derivePortfolioPositionMetrics(
    activePortfolio,
    snapshot,
  );
  const investedUsd = positionMetrics.investedUsd;
  const hasInvestedPositions = investedUsd > 0.5;
  const hasIdleCash =
    gatewayBalanceReady && (unifiedUsdc > 0.5 || unifiedEurc > 0.5);
  const showFaucet =
    !!wallet && gatewayBalanceReady && !hasInvestedPositions && !hasIdleCash;
  const showNoIdleCash =
    !!wallet &&
    gatewayBalanceReady &&
    hasInvestedPositions &&
    !hasIdleCash &&
    !!activePortfolio;
  const showDeploy =
    gatewayBalanceReady && deployableUsdc > 5 && !!activePortfolio;
  const maxTargetDriftPct = positionMetrics.maxDriftPct;
  const hasReviewableDrift = maxTargetDriftPct >= 5;
  const isFirstDeploy = investedUsd <= 5;
  const targetAllocations = targetAllocationsForPortfolio(activePortfolio);
  const usdcTargetWeight =
    targetAllocations.find((a) => a.symbol === "USDC")?.targetWeight ?? 0;
  const portfolioTitle = activePortfolio?.name ?? "Portfolio overview";

  if (!portfoliosLoaded || !activePortfolio) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center">
        <div className="max-w-sm border-brutal border-border-default bg-raised p-6 text-center">
          <Loader2 className="mx-auto mb-3 h-5 w-5 animate-spin text-accent-agent" />
          <h1 className="font-mono text-sm font-semibold text-text-hi">
            Loading dashboard
          </h1>
          <p className="mt-2 font-mono text-xs leading-relaxed text-text-lo">
            Loading your portfolio data.
          </p>
        </div>
      </div>
    );
  }

  const handleDeploy = async () => {
    if (!activePortfolio) return;
    setDeploying(true);
    setDeployError(null);
    setReviewMessage(null);
    try {
      const planned = await withTimeout(
        rebalanceApi.plan(activePortfolio.id),
        "Plan creation is taking longer than expected. Try again in a moment.",
      );
      setReviewPlan(planned);
      setReviewOpen(true);
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
    } finally {
      setDeploying(false);
    }
  };

  return (
    <motion.div
      initial="hidden"
      animate="visible"
      variants={stagger}
      className="max-w-[1400px] mx-auto space-y-6"
    >
      <motion.div
        variants={fadeUp}
        className="rounded-sharp border-brutal border-border-default bg-surface p-4 md:p-5"
      >
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <span className="border border-accent-agent/50 bg-accent-agent/10 px-2 py-1 text-[10px] font-mono uppercase text-accent-agent">
                Dashboard
              </span>
              <span className="max-w-full truncate border border-border-default bg-bg px-2 py-1 text-[10px] font-mono uppercase tracking-widest text-text-mut">
                Active portfolio:{" "}
                <span className="normal-case tracking-normal text-text-hi">
                  {portfolioTitle}
                </span>
              </span>
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
                {portfolioTitle}
              </h1>
            </div>
            <p className="mt-2 max-w-2xl text-xs font-mono leading-relaxed text-text-lo">
              {dashboardGuidance({
                gatewayBalanceUnavailable,
                gatewayBalanceStatus,
                showDeploy,
                showFaucet,
                showNoIdleCash,
                hasReviewableDrift,
                hasInvestedPositions,
              })}
            </p>
          </div>
          {wallet && (
            <div className="inline-flex min-h-9 shrink-0 items-center gap-2 self-start rounded-sharp border border-accent-agent/35 bg-accent-agent/5 px-3 font-mono text-[10px] uppercase tracking-widest text-accent-agent lg:self-auto">
              Account connected
            </div>
          )}
        </div>
      </motion.div>

      {gatewayBalanceUnavailable && (
        <motion.div
          variants={fadeUp}
          className="border-brutal border-warn/50 bg-warn/5 p-4 rounded-sharp"
        >
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
            <div className="flex items-start gap-3">
              <CircleAlert className="mt-0.5 h-5 w-5 shrink-0 text-warn" />
              <div>
                <p className="font-mono text-sm font-semibold text-text-hi">
                  Wallet balance could not be confirmed
                </p>
                <p className="mt-1 max-w-3xl font-mono text-xs leading-relaxed text-text-lo">
                  {gatewayBalanceError ??
                    "The balance check did not return a current wallet total."}{" "}
                  Cash actions stay hidden so an outage cannot look like a real
                  $0 wallet.
                </p>
                {hasReviewableDrift && (
                  <div className="mt-3 grid gap-2 border border-warn/40 bg-bg/70 p-3 font-mono text-xs md:grid-cols-[auto_1fr]">
                    <LockKeyhole className="h-4 w-4 text-warn" />
                    <div>
                      <p className="font-semibold text-text-hi">
                        Review is waiting for a wallet check
                      </p>
                      <p className="mt-1 leading-relaxed text-text-lo">
                        Aegis sees {maxTargetDriftPct.toFixed(1)}% target drift,
                        but it will not prepare trades until the current wallet
                        cash is confirmed.
                      </p>
                    </div>
                  </div>
                )}
              </div>
            </div>
            <div className="grid gap-2 sm:grid-cols-2 lg:min-w-[260px] lg:grid-cols-1">
              {hasReviewableDrift && (
                <div className="grid grid-cols-3 gap-2 text-center text-[10px] font-mono">
                  <StepBadge active label="1 Drift" />
                  <StepBadge active={false} label="2 Balance" />
                  <StepBadge active={false} label="3 Review" />
                </div>
              )}
              <a
                href="/wallets"
                className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 rounded-sharp border border-warn/40 bg-warn/10 px-3 font-mono text-xs font-semibold text-warn hover:bg-warn/15"
              >
                Open wallet status
              </a>
            </div>
          </div>
        </motion.div>
      )}

      {showFaucet && (
        <motion.div
          variants={fadeUp}
          className="border-brutal border-accent-agent/40 bg-accent-agent/5 p-4 rounded-sharp flex flex-wrap items-center justify-between gap-3"
        >
          <div>
            <p className="text-sm font-semibold text-text-hi font-mono">
              Empty wallet — add test USDC to start
            </p>
            <p className="text-xs text-text-lo font-mono mt-1">
              Adds test USDC to this account so you can review the first
              investment plan.
            </p>
          </div>
          <FaucetButton />
        </motion.div>
      )}

      {showNoIdleCash && (
        <motion.div
          variants={fadeUp}
          className="border-brutal border-border-default bg-raised p-4 md:p-5 rounded-sharp"
        >
          <div className="grid gap-4 lg:grid-cols-[1fr_auto] lg:items-center">
            <div className="min-w-0">
              <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
                {hasReviewableDrift
                  ? "Target drift detected · no wallet cash"
                  : "No wallet cash available"}
              </p>
              <h2 className="mt-1 text-lg font-mono font-semibold text-text-hi">
                {hasReviewableDrift
                  ? `${maxTargetDriftPct.toFixed(1)}% target drift needs review`
                  : `${formatCurrency(investedUsd)} is already invested`}
              </h2>
              <p className="text-xs text-text-lo font-mono mt-2 max-w-3xl leading-relaxed">
                {hasReviewableDrift
                  ? "Your wallet has no spare USDC, but the current mix no longer matches the target. Review the plan before any trade executes."
                  : "There is no spare USDC in the wallet right now. Add funds if you want Aegis to prepare a new move."}
              </p>
              {deployError && (
                <p className="text-xs text-risk font-mono mt-2">
                  {deployError}
                </p>
              )}
            </div>
            <div className="grid gap-3 lg:min-w-[320px]">
              <div className="grid gap-2 text-[10px] font-mono sm:grid-cols-3">
                <StepBadge active label="1 Invested" />
                <StepBadge active={hasReviewableDrift} label="2 Review" />
                <StepBadge active={false} label="3 Execute" />
              </div>
              {hasReviewableDrift && (
                <BrutalButton
                  variant="agent"
                  onClick={() => void handleDeploy()}
                  disabled={deploying}
                  className="w-full"
                >
                  {deploying ? (
                    <>
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                      Preparing review…
                    </>
                  ) : (
                    <>
                      <Rocket className="w-4 h-4 mr-2" />
                      Review plan
                    </>
                  )}
                </BrutalButton>
              )}
            </div>
          </div>
        </motion.div>
      )}

      {showDeploy && (
        <motion.div
          variants={fadeUp}
          className="border-brutal border-accent-pnl bg-accent-pnl/5 p-4 rounded-sharp shadow-brutal-sm md:p-5"
        >
          <div className="min-w-0">
            <p className="text-[10px] font-mono uppercase tracking-widest text-accent-pnl">
              Next step
            </p>
            <h2 className="mt-1 text-xl font-mono font-semibold text-text-hi">
              {isFirstDeploy
                ? `${formatCurrency(deployableUsdc)} USDC ready to review`
                : `${formatCurrency(deployableUsdc)} USDC ready to move`}
            </h2>
            <div className="mt-4 grid gap-2 font-mono text-xs sm:grid-cols-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_minmax(240px,320px)] lg:items-stretch">
              <MoneyStep active label="Wallet" value="Funded" />
              <MoneyStep active label="Review" value="You approve" />
              <MoneyStep label="Invested" value="After approval" />
              <div className="flex items-center sm:col-span-3 lg:col-span-1">
                <BrutalButton
                  variant="pnl"
                  onClick={() => void handleDeploy()}
                  disabled={deploying}
                  className="w-full py-2"
                >
                  {deploying ? (
                    <>
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                      Preparing plan…
                    </>
                  ) : (
                    <>
                      <Rocket className="w-4 h-4 mr-2" />
                      Review plan
                    </>
                  )}
                </BrutalButton>
              </div>
            </div>
            {(usdcTargetWeight > 0 || unifiedEurc > 0) && (
              <p className="mt-3 font-mono text-[11px] leading-relaxed text-text-lo">
                {usdcTargetWeight > 0 &&
                  `${usdcTargetWeight.toFixed(0)}% remains USDC reserve.`}
                {unifiedEurc > 0 &&
                  " EURC stays separate until a review includes it."}
              </p>
            )}
            {deployError && (
              <p className="text-xs text-risk font-mono mt-2">{deployError}</p>
            )}
            {reviewMessage && (
              <p className="mt-2 text-xs font-mono text-accent-agent">
                {reviewMessage}
              </p>
            )}
          </div>
        </motion.div>
      )}

      <motion.div
        variants={fadeUp}
        className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,240px),1fr))] items-start gap-4"
      >
        <PortfolioSummaryCard />
        <IdleCashCard />
        {!showDeploy && <AllocationChart />}
        <MarketOverview />
      </motion.div>

      <motion.div variants={fadeUp}>
        <TrustabilityCard />
      </motion.div>

      <motion.div variants={fadeUp}>
        <PerformanceChart />
      </motion.div>

      <motion.div
        variants={fadeUp}
        className="grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_minmax(320px,380px)] gap-6"
      >
        <AssetTable />
        <AgentReasoningFeed />
      </motion.div>

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
        onApproved={() => {
          setReviewOpen(false);
          setReviewMessage("Approved. Execution status will update live.");
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

function dashboardGuidance({
  gatewayBalanceUnavailable,
  gatewayBalanceStatus,
  showDeploy,
  showFaucet,
  showNoIdleCash,
  hasReviewableDrift,
  hasInvestedPositions,
}: {
  gatewayBalanceUnavailable: boolean;
  gatewayBalanceStatus: "idle" | "loading" | "ready" | "error";
  showDeploy: boolean;
  showFaucet: boolean;
  showNoIdleCash: boolean;
  hasReviewableDrift: boolean;
  hasInvestedPositions: boolean;
}) {
  if (gatewayBalanceUnavailable) {
    return "Wallet balance is unavailable, so cash actions are paused until the balance check succeeds.";
  }
  if (gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading") {
    return "Syncing balances before showing cash actions.";
  }
  if (showDeploy) {
    return "Cash is ready. Review and approve before anything moves.";
  }
  if (showFaucet) {
    return "Your wallet is empty. Add test USDC, then review the first plan.";
  }
  if (showNoIdleCash && hasReviewableDrift) {
    return "Your current holdings drifted from target. Review the plan before any trade runs.";
  }
  if (hasInvestedPositions) {
    return "Your portfolio is invested. Aegis is monitoring for drift and new review opportunities.";
  }
  return "Choose a target mix and add funds to start.";
}

function MoneyStep({
  active = false,
  label,
  value,
}: {
  active?: boolean;
  label: string;
  value: string;
}) {
  return (
    <div
      className={
        "min-h-16 border px-3 py-2 rounded-sharp " +
        (active
          ? "border-accent-pnl/50 bg-accent-pnl/10"
          : "border-border-default bg-bg/80")
      }
    >
      <p className="text-[9px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={
          "mt-1 text-sm font-semibold " +
          (active ? "text-accent-pnl" : "text-text-lo")
        }
      >
        {value}
      </p>
    </div>
  );
}

function StepBadge({ active, label }: { active: boolean; label: string }) {
  return (
    <span
      className={
        "flex min-h-8 items-center justify-center border px-2 py-1 text-center rounded-sharp " +
        (active
          ? "border-accent-pnl bg-accent-pnl text-black"
          : "border-border-default bg-raised text-text-mut")
      }
    >
      {label}
    </span>
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
