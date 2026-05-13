"use client";

import { motion } from "framer-motion";
import { PortfolioSummaryCard } from "@/components/dashboard/portfolio-summary-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { AssetTable } from "@/components/dashboard/asset-table";
import { AgentReasoningFeed } from "@/components/agent/reasoning-feed";
import { PerformanceChart } from "@/components/dashboard/performance-chart";
import { MarketOverview } from "@/components/dashboard/market-overview";

const stagger = {
  visible: { transition: { staggerChildren: 0.08 } },
};

const fadeUp = {
  hidden: { opacity: 0, y: 16 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.4, ease: "easeOut" } },
};

export default function DashboardPage() {
  return (
    <motion.div
      initial="hidden"
      animate="visible"
      variants={stagger}
      className="max-w-[1400px] mx-auto space-y-6"
    >
      {/* Top stats row */}
      <motion.div variants={fadeUp}>
        <h1 className="text-2xl font-bold mb-6 text-white">Portfolio Overview</h1>
      </motion.div>

      <motion.div variants={fadeUp} className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <PortfolioSummaryCard />
        <AllocationChart />
        <MarketOverview />
      </motion.div>

      {/* Performance chart */}
      <motion.div variants={fadeUp}>
        <PerformanceChart />
      </motion.div>

      {/* Assets + AI feed */}
      <motion.div variants={fadeUp} className="grid grid-cols-1 lg:grid-cols-[1fr_380px] gap-6">
        <AssetTable />
        <AgentReasoningFeed />
      </motion.div>
    </motion.div>
  );
}
