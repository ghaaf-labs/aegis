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
import { DiaryVisibilityToggle } from "@/components/settings/diary-visibility-toggle";
import { DigestOptIn } from "@/components/settings/digest-opt-in";
import { TrustabilityCard } from "@/components/dashboard/trustability-card";
import { LivePill } from "@/components/realtime/live-pill";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { Button } from "@/components/ui/button";
import { portfolioApi, rebalanceApi } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";
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

  const diaryQuery = useApiQuery(
    `portfolio.diaryPublic.${params?.portfolioId ?? ""}`,
    () => portfolioApi.getDiaryPublic(params!.portfolioId),
    { enabled: !!params?.portfolioId },
  );
  const [localDiaryPublic, setLocalDiaryPublic] = useState<boolean | null>(
    null,
  );
  const diaryPublic = localDiaryPublic ?? diaryQuery.data?.diaryPublic ?? false;

  const [storedEmail, setStoredEmail] = useState("");
  useEffect(() => {
    setStoredEmail(localStorage.getItem("aegis_email") ?? "");
  }, []);

  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const wallet = usePortfolioStore((s) => s.wallet);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const activePortfolio = useActivePortfolio();
  const router = useRouter();
  const [deploying, setDeploying] = useState(false);
  const [deployError, setDeployError] = useState<string | null>(null);

  const eurcUsd =
    snapshot?.assets.find((a) => a.symbol === "EURC")?.priceUsd ?? 1.085;
  const idleCashUsd = unifiedUsdc + unifiedEurc * eurcUsd;
  const investedUsd = activePortfolio?.totalValueUsd ?? 0;
  const showFaucet = !!wallet && unifiedUsdc === 0 && unifiedEurc === 0;
  const showDeploy = idleCashUsd > 5 && investedUsd <= 5 && !!activePortfolio;

  const handleDeploy = async () => {
    if (!activePortfolio) return;
    setDeploying(true);
    setDeployError(null);
    try {
      const planned = await rebalanceApi.plan(activePortfolio.id);
      router.push(`/rebalance/${planned.rebalanceId}`);
    } catch (e) {
      setDeployError(
        e instanceof Error ? e.message : "Could not build deploy plan",
      );
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
        className="flex items-center justify-between"
      >
        <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
          Portfolio Overview
        </h1>
        <LivePill />
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

      {showDeploy && (
        <motion.div
          variants={fadeUp}
          className="border-brutal border-accent-pnl/40 bg-accent-pnl/5 p-4 rounded-sharp flex flex-wrap items-center justify-between gap-3"
        >
          <div className="min-w-0">
            <p className="text-sm font-semibold text-text-hi font-mono">
              Deploy {formatCurrency(idleCashUsd)} idle cash into your target
              mix
            </p>
            <p className="text-xs text-text-lo font-mono mt-1">
              Builds a CCTP + Hooks plan that allocates Gateway USDC and EURC
              across your target weights. Review on the next screen before any
              on-chain move.
            </p>
            {deployError && (
              <p className="text-xs text-risk font-mono mt-2">{deployError}</p>
            )}
          </div>
          <Button
            size="sm"
            onClick={handleDeploy}
            disabled={deploying}
            className="bg-emerald-600 hover:bg-emerald-500 text-white"
          >
            {deploying ? (
              <>
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                Building plan…
              </>
            ) : (
              <>
                <Rocket className="w-4 h-4 mr-2" />
                Deploy idle cash
              </>
            )}
          </Button>
        </motion.div>
      )}

      <motion.div
        variants={fadeUp}
        className="grid grid-cols-1 md:grid-cols-3 gap-4"
      >
        <PortfolioSummaryCard />
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
        className="grid grid-cols-1 lg:grid-cols-[1fr_380px] gap-6"
      >
        <AssetTable />
        <AgentReasoningFeed />
      </motion.div>

      <motion.div
        variants={fadeUp}
        className="grid grid-cols-1 md:grid-cols-2 gap-4"
      >
        <DigestOptIn defaultEmail={storedEmail} />
        <div /> {/* spacer to keep grid alignment until more settings land */}
      </motion.div>

      <motion.div variants={fadeUp}>
        <DiaryVisibilityToggle
          key={`diary-${params?.portfolioId}-${diaryPublic}`}
          initialPublic={diaryPublic}
          onChange={async (next) => {
            if (!params?.portfolioId) return;
            const res = await portfolioApi.setDiaryPublic(
              params.portfolioId,
              next,
            );
            setLocalDiaryPublic(res.diaryPublic);
          }}
        />
      </motion.div>
    </motion.div>
  );
}
