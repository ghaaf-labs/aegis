"use client";

import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useParams, useRouter } from "next/navigation";
import { Loader2, Rocket } from "lucide-react";
import { PortfolioSummaryCard } from "@/components/dashboard/portfolio-summary-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { AssetTable } from "@/components/dashboard/asset-table";
import { AgentReasoningFeed } from "@/components/agent/reasoning-feed";
import { PerformanceChart } from "@/components/dashboard/performance-chart";
import { MarketOverview } from "@/components/dashboard/market-overview";
import { TrustabilityCard } from "@/components/dashboard/trustability-card";
import { IdleCashCard } from "@/components/dashboard/idle-cash-card";
import { DashboardTopology } from "@/components/dashboard/dashboard-topology";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { BrutalButton } from "@aegis/ui";
import { rebalanceApi } from "@/lib/api";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";

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
  const wallet = usePortfolioStore((s) => s.wallet);
  const activePortfolio = useActivePortfolio();
  const router = useRouter();
  const [deploying, setDeploying] = useState(false);
  const [deployError, setDeployError] = useState<string | null>(null);

  const deployableUsdc = unifiedUsdc;
  const investedUsd = activePortfolio?.totalValueUsd ?? 0;
  const hasInvestedPositions = investedUsd > 0.5;
  const hasIdleCash = unifiedUsdc > 0.5 || unifiedEurc > 0.5;
  const showFaucet = !!wallet && !hasInvestedPositions && !hasIdleCash;
  const showNoIdleCash =
    !!wallet && hasInvestedPositions && !hasIdleCash && !!activePortfolio;
  const showDeploy = deployableUsdc > 5 && !!activePortfolio;
  const isFirstDeploy = investedUsd <= 5;

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
      const friendly = /parse strategist proposal|json|JSON/i.test(raw)
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
        className="relative overflow-hidden border-brutal border-border-default bg-surface p-4 md:p-5 rounded-sharp"
      >
        <DashboardTopology />
        <div className="absolute inset-0 bg-gradient-to-r from-bg via-bg/85 to-bg/35" />
        <div className="relative z-10 flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
                {activePortfolio?.name ?? "Portfolio Overview"}
              </h1>
              <span className="border border-accent-agent/50 bg-accent-agent/10 px-2 py-1 text-[10px] font-mono uppercase text-accent-agent">
                Portfolio command
              </span>
            </div>
            <p className="mt-2 max-w-2xl text-xs font-mono leading-relaxed text-text-lo">
              One place for the truth: invested positions, idle Gateway cash,
              agent decisions, and rebalance approvals. Nothing moves until you
              review a plan.
            </p>
          </div>
          <dl className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:min-w-[520px]">
            <HeaderStat label="Invested" value={formatCurrency(investedUsd)} />
            <HeaderStat
              label="Idle USDC"
              value={formatCurrency(deployableUsdc)}
              tone={deployableUsdc > 5 ? "pnl" : "muted"}
            />
            <HeaderStat
              label="EURC"
              value={formatCurrency(unifiedEurc)}
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

      {showFaucet && (
        <motion.div
          variants={fadeUp}
          className="border-brutal border-accent-agent/40 bg-accent-agent/5 p-4 rounded-sharp flex flex-wrap items-center justify-between gap-3"
        >
          <div>
            <p className="text-sm font-semibold text-text-hi font-mono">
              Empty wallet — fund with testnet USDC to drive the agent
            </p>
            <p className="text-xs text-text-lo font-mono mt-1">
              Claims 100 USDC from Circle&apos;s Arc Sepolia faucet. Required
              before rebalances + agent decisions move real positions.
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
                Portfolio is invested · no idle USDC
              </p>
              <h2 className="mt-1 text-lg font-mono font-semibold text-text-hi">
                {formatCurrency(investedUsd)} is already in positions
              </h2>
              <p className="text-xs text-text-lo font-mono mt-2 max-w-3xl leading-relaxed">
                Your Gateway wallet has no deployable USDC right now, so there
                is nothing for Deploy to move. Use Review rebalance when target
                weights drift, or fund the wallet if you want the agent to
                invest new cash.
              </p>
            </div>
            <div className="grid gap-2 text-[10px] font-mono sm:grid-cols-3 lg:min-w-[320px]">
              <StepBadge active label="1 Invested" />
              <StepBadge active={false} label="2 Waiting" />
              <StepBadge active={false} label="3 Fund cash" />
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
                  : "Wallet cash available · review needed"}
              </p>
              <h2 className="mt-1 text-lg font-mono font-semibold text-text-hi">
                {isFirstDeploy
                  ? `You have ${formatCurrency(deployableUsdc)} USDC ready to invest`
                  : `${formatCurrency(deployableUsdc)} USDC is still idle in your wallet`}
              </h2>
              <p className="text-xs text-text-lo font-mono mt-2 max-w-3xl leading-relaxed">
                {isFirstDeploy
                  ? "Right now the USDC is safe in your Circle Gateway wallet. It is not in BTC, ETH, SOL, or USYC yet."
                  : `${formatCurrency(investedUsd)} is already in positions. The remaining USDC is not following the target mix yet.`}{" "}
                Review the exact USDC deployment first; no trade executes until
                you approve the next screen.
                {unifiedEurc > 0 && (
                  <>
                    {" "}
                    EURC remains visible in Wallet until the StableFX deploy
                    rail is enabled.
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
    <div className="border border-border-default bg-bg/80 px-3 py-2 rounded-sharp backdrop-blur-sm">
      <dt className="text-[10px] font-mono uppercase text-text-mut">{label}</dt>
      <dd
        className={`mt-1 truncate text-sm font-mono font-semibold tabular-nums ${valueClass}`}
      >
        {value}
      </dd>
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
