"use client";
import { useState } from "react";
import { CircleAlert, Plus, RefreshCw, ShieldCheck } from "lucide-react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { BrutalButton } from "@aegis/ui";
import { AssetTable } from "@/components/dashboard/asset-table";
import { RebalanceModal } from "@/components/portfolio/rebalance-modal";
import { RiskScoreCard } from "@/components/portfolio/risk-score-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";

export default function PortfolioPage() {
  const router = useRouter();
  const [rebalanceOpen, setRebalanceOpen] = useState(false);
  const portfolio = useActivePortfolio();
  const wallet = usePortfolioStore((s) => s.wallet);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const reviewReady = !!wallet && gatewayBalanceStatus === "ready";
  const readiness = rebalanceReadinessCopy(
    !!wallet,
    gatewayBalanceStatus,
    gatewayBalanceError,
  );

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
            Review positions, targets, and wallet cash before approving a move.
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
          <BrutalButton
            variant={reviewReady ? "agent" : "ghost"}
            onClick={() => setRebalanceOpen(true)}
          >
            {reviewReady ? (
              <RefreshCw className="w-4 h-4 mr-2" />
            ) : (
              <CircleAlert className="w-4 h-4 mr-2" />
            )}
            {reviewReady ? "Review rebalance" : "Fix setup"}
          </BrutalButton>
        </div>
      </div>

      <section
        aria-label="Rebalance setup"
        className={`rounded-sharp border-brutal p-4 md:p-5 ${
          reviewReady
            ? "border-accent-agent/40 bg-accent-agent/5"
            : "border-warn/50 bg-warn/5"
        }`}
      >
        <div className="flex items-start gap-3">
          <div
            className={`mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-sharp border-brutal border-black ${
              reviewReady ? "bg-accent-agent" : "bg-warn"
            }`}
          >
            {reviewReady ? (
              <ShieldCheck className="h-4 w-4 text-black" />
            ) : (
              <CircleAlert className="h-4 w-4 text-black" />
            )}
          </div>
          <div>
            <p
              className={`font-mono text-[10px] uppercase tracking-widest ${
                reviewReady ? "text-accent-agent" : "text-warn"
              }`}
            >
              Before review
            </p>
            <h2 className="mt-1 font-mono text-lg font-semibold text-text-hi">
              {readiness.title}
            </h2>
            <p className="mt-2 max-w-3xl font-mono text-xs leading-relaxed text-text-lo">
              {readiness.copy}
            </p>
          </div>
        </div>
        <div className="mt-4 flex flex-col gap-2 sm:flex-row">
          <BrutalButton
            variant={reviewReady ? "agent" : "ghost"}
            onClick={() => setRebalanceOpen(true)}
          >
            {reviewReady ? (
              <RefreshCw className="w-4 h-4 mr-2" />
            ) : (
              <CircleAlert className="w-4 h-4 mr-2" />
            )}
            {reviewReady ? "Build review plan" : "Open setup details"}
          </BrutalButton>
          {!wallet && (
            <Link
              href="/wallets"
              className="inline-flex min-h-10 items-center justify-center rounded-sharp border border-warn/40 px-3 font-mono text-xs font-semibold text-warn hover:bg-warn/10"
            >
              Check account setup
            </Link>
          )}
          {gatewayBalanceStatus === "error" && (
            <Link
              href="/wallets"
              className="inline-flex min-h-10 items-center justify-center rounded-sharp border border-warn/40 px-3 font-mono text-xs font-semibold text-warn hover:bg-warn/10"
            >
              Open wallet status
            </Link>
          )}
        </div>
      </section>

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

type GatewayBalanceStatus = "idle" | "loading" | "ready" | "error";

function rebalanceReadinessCopy(
  hasWallet: boolean,
  gatewayBalanceStatus: GatewayBalanceStatus,
  gatewayBalanceError: string | null,
) {
  if (!hasWallet) {
    return {
      title: "Finish setup before a rebalance review",
      copy: "Aegis needs your wallet ready and a current cash check before it can build a review.",
    };
  }
  if (gatewayBalanceStatus === "error") {
    return {
      title: "Rebalance is paused — wallet cash is unknown",
      copy:
        gatewayBalanceError ??
        "The balance check didn't confirm your cash. Aegis won't treat an unavailable balance as $0 or plan against stale cash.",
    };
  }
  if (gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading") {
    return {
      title: "Checking wallet cash…",
      copy: "Aegis is confirming your balance. The review unlocks once wallet cash is known.",
    };
  }
  return {
    title: "Ready to build a rebalance review",
    copy: "Your wallet and cash are confirmed. Build a review from current positions, targets, and wallet cash. You approve before anything runs.",
  };
}
