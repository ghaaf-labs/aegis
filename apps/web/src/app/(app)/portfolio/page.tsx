"use client";
import { useState, type ReactNode } from "react";
import {
  Activity,
  Banknote,
  CircleAlert,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Target,
  Wallet,
} from "lucide-react";
import Link from "next/link";
import { BrutalButton, ProvenanceLine } from "@aegis/ui";
import { AssetTable } from "@/components/dashboard/asset-table";
import { RebalanceModal } from "@/components/portfolio/rebalance-modal";
import { RiskScoreCard } from "@/components/portfolio/risk-score-card";
import { AllocationChart } from "@/components/dashboard/allocation-chart";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { agentApi } from "@/lib/api";
import {
  deriveDashboardBalanceModel,
  type DashboardBalanceModel,
} from "@/lib/dashboard-balance-model";
import { formatCurrency } from "@/lib/utils";

export default function PortfolioPage() {
  const [rebalanceOpen, setRebalanceOpen] = useState(false);
  const [reproposing, setReproposing] = useState(false);
  const portfolio = useActivePortfolio();
  const decisions = usePortfolioStore((s) => s.decisions);
  const wallet = usePortfolioStore((s) => s.wallet);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const gatewayBalanceUpdatedAt = usePortfolioStore(
    (s) => s.gatewayBalanceUpdatedAt,
  );
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const tokenBalancesByChain = usePortfolioStore((s) => s.tokenBalancesByChain);
  const reviewReady = !!wallet && gatewayBalanceStatus === "ready";
  const latestProposal = decisions.find(
    (d) => d.portfolioId === portfolio?.id && d.kind === "allocation_proposal",
  );
  const balanceModel = deriveDashboardBalanceModel({
    portfolio,
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

  const handleRepropose = async () => {
    if (!portfolio) return;
    setReproposing(true);
    try {
      await agentApi.proposeAllocation(portfolio.id);
    } catch {
      /* the dashboard surfaces the resulting proposal via the store/SSE */
    } finally {
      setReproposing(false);
    }
  };
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
          <Link
            href="/onboarding"
            className="text-accent-agent hover:underline"
          >
            Set your goal
          </Link>{" "}
          and the agent designs the allocation.
        </p>
      </div>
    );
  }

  return (
    <div className="mx-auto w-full max-w-[1280px] space-y-5 md:space-y-6">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
            Agent-managed portfolio
          </h1>
          <p className="text-sm text-text-lo mt-1">
            Live wallet cash and positions reconciled against the agent target.
          </p>
          <p className="mt-2 font-mono text-[11px] text-text-mut">
            Agent decided this allocation
            {latestProposal?.modelSlug
              ? ` · via ${latestProposal.modelSlug}`
              : ""}
            {latestProposal?.regime ? ` · ${latestProposal.regime}` : ""}
          </p>
        </div>
        <div className="grid gap-2 sm:grid-cols-2 lg:min-w-[360px]">
          <BrutalButton
            variant="ghost"
            onClick={() => void handleRepropose()}
            disabled={reproposing}
            className="w-full"
          >
            <Sparkles className="w-4 h-4 mr-2" />
            {reproposing ? "Re-proposing…" : "Re-propose"}
          </BrutalButton>
          <BrutalButton
            variant={reviewReady ? "agent" : "ghost"}
            onClick={() => setRebalanceOpen(true)}
            className="w-full"
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

      <PortfolioSummary model={balanceModel} />

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
        <div className="mt-4 grid gap-2 sm:grid-cols-[minmax(0,220px)_minmax(0,220px)]">
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

      <div className="grid min-w-0 grid-cols-1 gap-5 md:gap-6">
        <AssetTable model={balanceModel} />

        <div className="grid min-w-0 grid-cols-1 gap-5 md:gap-6 lg:grid-cols-2">
          <AllocationChart compact model={balanceModel} />
          <RiskScoreCard model={balanceModel} />
        </div>
      </div>

      <RebalanceModal
        open={rebalanceOpen}
        onClose={() => setRebalanceOpen(false)}
      />
    </div>
  );
}

function PortfolioSummary({ model }: { model: DashboardBalanceModel }) {
  return (
    <section
      aria-label="Portfolio totals"
      className="overflow-hidden rounded-sharp border-brutal border-border-default bg-surface"
    >
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4">
        <SummaryCell
          icon={<Activity className="h-4 w-4 text-accent-agent" />}
          label="Net worth"
          value={formatCurrency(model.netWorthUsd)}
          detail="wallet cash + positions"
        />
        <SummaryCell
          icon={<Banknote className="h-4 w-4 text-accent-pnl" />}
          label="Invested"
          value={formatCurrency(model.investedUsd)}
          detail={
            model.hasInvestedPositions ? "confirmed exposure" : "no live fills"
          }
        />
        <SummaryCell
          icon={<Wallet className="h-4 w-4 text-accent-pnl" />}
          label="Wallet cash"
          value={
            model.walletBalanceUnavailable
              ? "Unknown"
              : formatCurrency(model.walletValueUsd)
          }
          detail={
            model.walletBalanceUnavailable
              ? "balance check failed"
              : `${formatCurrency(model.reserveUsd)} reserve target`
          }
        />
        <SummaryCell
          icon={<Target className="h-4 w-4 text-warn" />}
          label="Deployable"
          value={formatCurrency(model.deployableUsd)}
          detail={model.status.label}
          tone={model.status.tone}
        />
      </div>
      <div className="border-t border-border-default px-4 py-2 sm:px-5">
        <ProvenanceLine
          source="Circle balances + execution ledger"
          freshness={
            model.gatewayBalanceUpdatedAt
              ? "live"
              : model.walletBalanceLoading
                ? "syncing"
                : "current"
          }
        />
      </div>
    </section>
  );
}

function SummaryCell({
  detail,
  icon,
  label,
  tone = "default",
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  tone?: DashboardBalanceModel["status"]["tone"] | "default";
  value: string;
}) {
  return (
    <div className="min-w-0 border-b border-border-default p-4 md:[&:nth-child(odd)]:border-r xl:border-b-0 xl:border-r xl:last:border-r-0">
      <div className="flex items-center gap-2">
        {icon}
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p
        className={`mt-3 truncate font-mono text-2xl font-semibold tabular-nums ${summaryToneClass(
          tone,
        )}`}
      >
        {value}
      </p>
      <p className="mt-1 truncate font-mono text-[11px] text-text-lo">
        {detail}
      </p>
    </div>
  );
}

function summaryToneClass(
  tone: DashboardBalanceModel["status"]["tone"] | "default",
) {
  switch (tone) {
    case "agent":
      return "text-accent-agent";
    case "pnl":
      return "text-accent-pnl";
    case "risk":
      return "text-risk";
    case "warn":
      return "text-warn";
    case "muted":
      return "text-text-mut";
    default:
      return "text-text-hi";
  }
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
