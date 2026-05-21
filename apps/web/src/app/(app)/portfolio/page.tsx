"use client";
import { useState } from "react";
import { Plus, RefreshCw } from "lucide-react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { BrutalButton } from "@aegis/ui";
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
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
            My Portfolio
          </h1>
          <p className="text-sm text-text-lo mt-1">
            Review invested positions, target drift, and the next rebalance plan
            before anything runs.
          </p>
        </div>
        <div className="flex gap-3">
          <BrutalButton
            variant="ghost"
            onClick={() => router.push("/onboarding")}
          >
            <Plus className="w-4 h-4 mr-2" />
            Change target
          </BrutalButton>
          <BrutalButton variant="agent" onClick={() => setRebalanceOpen(true)}>
            <RefreshCw className="w-4 h-4 mr-2" />
            Review rebalance
          </BrutalButton>
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
    </div>
  );
}
