"use client";
import { useState } from "react";
import { motion } from "framer-motion";
import { Plus, RefreshCw } from "lucide-react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { AssetTable } from "@/components/dashboard/asset-table";
import { RebalanceModal } from "@/components/portfolio/rebalance-modal";
import { RiskScoreCard } from "@/components/portfolio/risk-score-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { useActivePortfolio } from "@/stores/portfolio";

export default function PortfolioPage() {
  const router = useRouter();
  const [rebalanceOpen, setRebalanceOpen] = useState(false);
  const portfolio = useActivePortfolio();

  if (!portfolio) {
    return (
      <div className="flex flex-col items-center justify-center min-h-[40vh] text-center space-y-3">
        <p className="text-sm font-mono text-text-lo">No portfolio selected.</p>
        <p className="text-xs font-mono text-text-mut">
          <Link href="/onboarding" className="text-accent-pnl hover:underline">
            Create a portfolio
          </Link>{" "}
          or{" "}
          <Link
            href="/strategies"
            className="text-accent-agent hover:underline"
          >
            adopt a strategy
          </Link>{" "}
          to get started.
        </p>
      </div>
    );
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4 }}
      className="max-w-[1400px] mx-auto space-y-6"
    >
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">My Portfolio</h1>
          <p className="text-sm text-gray-400 mt-0.5">
            Manage allocations and trigger rebalancing
          </p>
        </div>
        <div className="flex gap-3">
          <Button
            variant="outline"
            size="sm"
            className="border-white/10 text-gray-300 hover:bg-white/5"
            onClick={() => router.push("/onboarding")}
            title="Add new asset via onboarding"
          >
            <Plus className="w-4 h-4 mr-2" />
            Add asset
          </Button>
          <Button
            size="sm"
            onClick={() => setRebalanceOpen(true)}
            className="bg-blue-600 hover:bg-blue-500"
          >
            <RefreshCw className="w-4 h-4 mr-2" />
            Rebalance
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-6">
        <div className="space-y-6">
          <AssetTable />
        </div>
        <div className="space-y-4">
          <AllocationChart compact />
          <RiskScoreCard />
        </div>
      </div>

      <RebalanceModal
        open={rebalanceOpen}
        onClose={() => setRebalanceOpen(false)}
      />
    </motion.div>
  );
}
