"use client";

import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useParams } from "next/navigation";
import { PortfolioSummaryCard } from "@/components/dashboard/portfolio-summary-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { AssetTable } from "@/components/dashboard/asset-table";
import { AgentReasoningFeed } from "@/components/agent/reasoning-feed";
import { PerformanceChart } from "@/components/dashboard/performance-chart";
import { MarketOverview } from "@/components/dashboard/market-overview";
import { DiaryVisibilityToggle } from "@/components/settings/diary-visibility-toggle";
import { usePortfolioStore } from "@/stores/portfolio";

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
  const [diaryPublic, setDiaryPublic] = useState(false);

  useEffect(() => {
    if (params?.portfolioId) setActive(params.portfolioId);
  }, [params?.portfolioId, setActive]);

  return (
    <motion.div
      initial="hidden"
      animate="visible"
      variants={stagger}
      className="max-w-[1400px] mx-auto space-y-6"
    >
      <motion.div variants={fadeUp}>
        <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
          Portfolio Overview
        </h1>
      </motion.div>

      <motion.div
        variants={fadeUp}
        className="grid grid-cols-1 md:grid-cols-3 gap-4"
      >
        <PortfolioSummaryCard />
        <AllocationChart />
        <MarketOverview />
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

      <motion.div variants={fadeUp}>
        <DiaryVisibilityToggle
          initialPublic={diaryPublic}
          onChange={async (next) => {
            // Backend wiring lands in Sprint 4 (settings PATCH route).
            // Component is rendered here so the surface is reachable and
            // the diary opt-in flow is discoverable from the dashboard.
            setDiaryPublic(next);
          }}
        />
      </motion.div>
    </motion.div>
  );
}
