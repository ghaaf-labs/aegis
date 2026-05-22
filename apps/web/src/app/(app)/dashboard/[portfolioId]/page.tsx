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
import { ValueFlowCard } from "@/components/dashboard/value-flow-card";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { BrutalButton } from "@aegis/ui";
import { rebalanceApi } from "@/lib/api";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";

const stagger = { visible: { transition: { staggerChildren: 0.08 } } };
const fadeUp = {
  hidden: { opacity: 0, y: 16 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.4, ease: "easeOut" } },
};

/**
 * Multi-portfolio dashboard. Reads `portfolioId` from the route, marks it
 * active in the Zustand store, and renders the same dashboard surface as the
 * Sprint 1 single-portfolio view. The header switcher (in components/layout)
 * lets the user pivot between portfolios without a full reload.
 */
export default function PortfolioDashboardPage() {
  const params = useParams<{ portfolioId: string }>();
  const setActive = usePortfolioStore((s) => s.setActivePortfolio);

  useEffect(() => {
    if (params?.portfolioId) setActive(params.portfolioId);
    // Allocation hydration is handled by PortfolioLoader (mounted in the
    // (app) layout) — it watches activePortfolioId and re-fetches detail.
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
  const targetSymbols =
    activePortfolio?.allocations
      ?.filter((a) => a.targetWeight > 0 && a.symbol !== "USDC")
      .map((a) => a.symbol) ?? [];
  const usdcTargetWeight =
    activePortfolio?.allocations?.find((a) => a.symbol === "USDC")
      ?.targetWeight ?? 0;
  const targetAssetText =
    targetSymbols.length > 0
      ? formatAssetList(targetSymbols)
      : "the target mix";
  const portfolioTitle = activePortfolio?.name ?? "Portfolio overview";

  if (!portfoliosLoaded || !activePortfolio) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center">
        <div className="max-w-sm border-brutal border-border-default bg-raised p-6 text-center">
          <Loader2 className="mx-auto mb-3 h-5 w-5 animate-spin text-accent-agent" />
          <h1 className="font-mono text-sm font-semibold text-text-hi">
            Opening your dashboard
          </h1>
          <p className="mt-2 font-mono text-xs leading-relaxed text-text-lo">
            If no portfolio exists yet, Aegis will send you back to portfolio
            setup.
          </p>
        </div>
      </div>
    );
  }

  const handleDeploy = async () => {
    if (!activePortfolio) return;
    setDeploying(true);
    setDeployError(null);
    try {
      const planned = await withTimeout(
        rebalanceApi.plan(activePortfolio.id),
        "Plan creation is taking longer than expected. Try again in a moment.",
      );
      router.push(`/rebalance/${planned.rebalanceId}`);
    } catch (e) {
      const raw =
        e instanceof Error ? e.message : "Could not build deploy plan";
      // The strategist occasionally returns malformed JSON; the backend then
      // raises a 500 with the raw model output in the body. Dumping that into
      // the UI looks like a crash. Map known signatures to a friendlier
      // "agent hiccup, try again" message.
      const cleaned = raw
        .replace(/^\d{3}:\s*/, "")
        .replace(/^conflict:\s*/i, "");
      const friendly = cleaned
        .toLowerCase()
        .includes("no rebalance plan was created")
        ? cleaned
        : /parse strategist proposal|json|JSON/i.test(raw)
          ? "Agent had a formatting hiccup. Click Review deployment again — the second pass usually succeeds."
          : raw;
      setDeployError(friendly);
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
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
                {portfolioTitle}
              </h1>
              <span className="border border-accent-agent/50 bg-accent-agent/10 px-2 py-1 text-[10px] font-mono uppercase text-accent-agent">
                Dashboard
              </span>
            </div>
            <p className="mt-2 max-w-2xl text-xs font-mono leading-relaxed text-text-lo">
              See what is still cash, what is invested, and what needs your
              approval. Aegis only moves money after you review and confirm the
              next action.
            </p>
          </div>
          <dl className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:min-w-[520px]">
            <HeaderStat label="Invested" value={formatCurrency(investedUsd)} />
            <HeaderStat
              label="Idle USDC"
              value={
                gatewayBalanceUnavailable
                  ? "Unavailable"
                  : formatCurrency(deployableUsdc)
              }
              tone={deployableUsdc > 5 ? "pnl" : "muted"}
            />
            <HeaderStat
              label="EURC cash"
              value={
                gatewayBalanceUnavailable
                  ? "Unavailable"
                  : `€${unifiedEurc.toFixed(2)}`
              }
              tone={unifiedEurc > 0 ? "pnl" : "muted"}
            />
            <HeaderStat
              label="Wallet"
              value={wallet ? "Connected" : "Pending"}
              tone={wallet ? "agent" : "muted"}
            />
          </dl>
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
                        Rebalance is waiting for a wallet check
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
                  ? "Your wallet has no spare USDC, but the current mix no longer matches the target. Review a rebalance before any trade executes."
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
                      Review rebalance
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
          className="border-brutal border-accent-pnl bg-accent-pnl/5 p-4 md:p-5 rounded-sharp shadow-brutal-sm"
        >
          <div className="grid gap-4 lg:grid-cols-[1fr_auto] lg:items-center">
            <div className="min-w-0">
              <p className="text-[10px] font-mono uppercase tracking-widest text-accent-pnl">
                {isFirstDeploy
                  ? "Wallet funded · investment not started"
                  : "Wallet cash available · approval needed"}
              </p>
              <h2 className="mt-1 text-lg font-mono font-semibold text-text-hi">
                {isFirstDeploy
                  ? `You have ${formatCurrency(deployableUsdc)} USDC ready to invest`
                  : `${formatCurrency(deployableUsdc)} USDC is still idle in your wallet`}
              </h2>
              <p className="text-xs text-text-lo font-mono mt-2 max-w-3xl leading-relaxed">
                {isFirstDeploy
                  ? `Right now the USDC is still cash in your wallet. It has not been moved into ${targetAssetText} yet.`
                  : `${formatCurrency(investedUsd)} is already invested. The remaining USDC is still cash and not following the target mix yet.`}{" "}
                Review the exact changes first; no trade executes until you
                approve the next screen.
                {usdcTargetWeight > 0 && (
                  <>
                    {" "}
                    The {usdcTargetWeight.toFixed(0)}% USDC target stays as a
                    cash reserve.
                  </>
                )}
                {unifiedEurc > 0 && (
                  <>
                    {" "}
                    Existing EURC wallet cash stays separate until you approve a
                    move for it.
                  </>
                )}
              </p>
              {deployError && (
                <p className="text-xs text-risk font-mono mt-2">
                  {deployError}
                </p>
              )}
            </div>
            <div className="grid gap-3 sm:grid-cols-[1fr_auto] lg:grid-cols-1 lg:min-w-[260px]">
              <div className="grid grid-cols-3 gap-2 text-center text-[10px] font-mono">
                <StepBadge
                  active
                  label={isFirstDeploy ? "1 Funded" : "1 Cash idle"}
                />
                <StepBadge active={false} label="2 Review" />
                <StepBadge
                  active={!isFirstDeploy}
                  label={isFirstDeploy ? "3 Invested" : "3 Deployed"}
                />
              </div>
              <BrutalButton
                variant="pnl"
                onClick={() => void handleDeploy()}
                disabled={deploying}
                className="w-full"
              >
                {deploying ? (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                    Preparing plan…
                  </>
                ) : (
                  <>
                    <Rocket className="w-4 h-4 mr-2" />
                    {isFirstDeploy
                      ? "Review investment plan"
                      : "Review USDC deployment"}
                  </>
                )}
              </BrutalButton>
            </div>
          </div>
        </motion.div>
      )}

      <motion.div variants={fadeUp}>
        <ValueFlowCard
          portfolio={activePortfolio}
          idleUsdc={unifiedUsdc}
          idleEurc={unifiedEurc}
          investedUsd={investedUsd}
          walletCashStatus={gatewayBalanceStatus}
        />
      </motion.div>

      <motion.div
        variants={fadeUp}
        className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4"
      >
        <PortfolioSummaryCard />
        <IdleCashCard />
        <AllocationChart />
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

function HeaderStat({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "pnl" | "agent" | "muted";
}) {
  const valueClass =
    tone === "pnl"
      ? "text-accent-pnl"
      : tone === "agent"
        ? "text-accent-agent"
        : tone === "muted"
          ? "text-text-lo"
          : "text-text-hi";

  return (
    <div className="border border-border-default bg-bg/90 px-3 py-2 rounded-sharp">
      <dt className="text-[10px] font-mono uppercase text-text-mut">{label}</dt>
      <dd
        className={`mt-1 truncate text-sm font-mono font-semibold tabular-nums ${valueClass}`}
      >
        {value}
      </dd>
    </div>
  );
}

function formatAssetList(symbols: string[]) {
  const unique = Array.from(new Set(symbols));
  if (unique.length === 0) return "";
  if (unique.length === 1) return unique[0] ?? "";
  if (unique.length === 2) return `${unique[0]} and ${unique[1]}`;
  return `${unique.slice(0, -1).join(", ")}, and ${unique[unique.length - 1]}`;
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
