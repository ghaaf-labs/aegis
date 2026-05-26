"use client";

import Link from "next/link";
import type { ReactNode } from "react";
import {
  Activity,
  ArrowRight,
  CircleAlert,
  Loader2,
  Rocket,
  ShieldCheck,
  Sparkles,
  Wallet,
} from "lucide-react";
import {
  BrutalButton,
  BrutalCard as Card,
  BrutalCardBody as CardContent,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  ProvenanceLine,
} from "@aegis/ui";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { TokenBadge } from "@/components/dashboard/token-badge";
import type {
  DashboardBalanceModel,
  DashboardChainExposure,
  DashboardTokenExposure,
} from "@/lib/dashboard-balance-model";
import { isTradeableSleeve } from "@/lib/route-capabilities";
import { idleUsdcConsolidation } from "@/lib/wallet-routes";
import { cn, formatCurrency, timeAgo } from "@/lib/utils";

interface AssetControlTowerProps {
  model: DashboardBalanceModel;
  executionProgress?: ExecutionProgressSummary | null;
  onReviewPlan: () => void;
  reviewPlanLoading: boolean;
  onDesignAllocation: () => void;
  onOpenProposal: () => void;
  designLoading: boolean;
  designError: boolean;
  deployError: string | null;
  reviewMessage: string | null;
  proposalPending: boolean;
}

export interface ExecutionProgressSummary {
  rebalanceId: string;
  status: string;
  completedLegs: number;
  totalLegs: number;
  failureReason?: string | null;
}

export function AssetControlTower({
  model,
  executionProgress,
  onReviewPlan,
  reviewPlanLoading,
  onDesignAllocation,
  onOpenProposal,
  designLoading,
  designError,
  deployError,
  reviewMessage,
  proposalPending,
}: AssetControlTowerProps) {
  return (
    <Card data-testid="asset-control-tower" className="overflow-hidden">
      <CardHeader className="min-h-[56px]">
        <CardTitle className="flex min-w-0 items-center gap-2">
          <Wallet className="h-3.5 w-3.5 shrink-0 text-accent-pnl" />
          <span className="truncate">Asset Control Tower</span>
        </CardTitle>
        <span className="hidden font-mono text-[10px] text-text-mut md:block">
          Operations dashboard with exposure and actions
        </span>
      </CardHeader>

      <CardContent className="p-0 font-mono">
        <KpiRail model={model} executionProgress={executionProgress} />

        <div className="grid gap-0 md:grid-cols-2 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(320px,0.9fr)]">
          <ExposurePanel
            tokens={model.tokens}
            netWorthUsd={model.netWorthUsd}
          />
          <ChainPanel chains={model.chains} />
          <ActionPanel
            model={model}
            onReviewPlan={onReviewPlan}
            reviewPlanLoading={reviewPlanLoading}
            onDesignAllocation={onDesignAllocation}
            onOpenProposal={onOpenProposal}
            designLoading={designLoading}
            designError={designError}
            deployError={deployError}
            reviewMessage={reviewMessage}
            proposalPending={proposalPending}
            executionProgress={executionProgress}
          />
        </div>

        <QuickStats model={model} />

        <div className="px-4 py-3">
          <ProvenanceLine
            source="Circle balances + execution ledger"
            freshness={freshness(model)}
            className={model.walletBalanceUnavailable ? "text-warn" : undefined}
          />
        </div>
      </CardContent>
    </Card>
  );
}

function KpiRail({
  model,
  executionProgress,
}: {
  model: DashboardBalanceModel;
  executionProgress?: ExecutionProgressSummary | null;
}) {
  const status = executionStatusKpi(executionProgress) ?? model.status;
  return (
    <div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
      <KpiCell label="Net worth" value={formatCurrency(model.netWorthUsd)} />
      <KpiCell label="Invested" value={formatCurrency(model.investedUsd)} />
      <KpiCell
        label="Wallet cash"
        value={formatCurrency(model.walletValueUsd)}
      />
      <KpiCell
        label={`Reserve (${model.reservePct.toFixed(0)}%)`}
        value={formatCurrency(model.reserveUsd)}
        detail={model.reservePct > 0 ? "Locked" : "None"}
      />
      <KpiCell
        label="Deployable"
        value={formatCurrency(model.deployableUsd)}
        detail={model.deployableUsd > 0.5 ? "Ready" : "None"}
        tone="pnl"
      />
      <KpiCell
        label="Status"
        value={status.label}
        detail={status.detail}
        tone={status.tone}
      />
    </div>
  );
}

function KpiCell({
  label,
  value,
  detail,
  tone = "default",
}: {
  label: string;
  value: string;
  detail?: string;
  tone?: "default" | "pnl" | "agent" | "warn" | "risk" | "muted";
}) {
  return (
    <div className="min-h-[82px] border-b border-r border-border-default px-4 py-3 [&:nth-child(2n)]:border-r-0 lg:[&:nth-child(2n)]:border-r lg:[&:nth-child(3n)]:border-r-0 xl:[&:nth-child(3n)]:border-r xl:[&:nth-child(6n)]:border-r-0">
      <p className="truncate text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={cn(
          "mt-1 line-clamp-2 text-base font-semibold tabular-nums",
          toneClass(tone),
        )}
        title={value}
      >
        {value}
      </p>
      {detail && (
        <p
          className="mt-1 line-clamp-2 text-[10px] leading-relaxed text-text-lo"
          title={detail}
        >
          {detail}
        </p>
      )}
    </div>
  );
}

function ExposurePanel({
  tokens,
  netWorthUsd,
}: {
  tokens: DashboardTokenExposure[];
  netWorthUsd: number;
}) {
  const activeTokens = tokens.filter((token) => token.totalUsd > 0.005);
  const visible = activeTokens.slice(0, 7);
  const hidden = Math.max(0, activeTokens.length - visible.length);
  const maxWeight = Math.max(1, ...visible.map((token) => token.weightPct));

  return (
    <section className="min-w-0 border-b border-border-default p-4 md:border-b-0 md:border-r">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        Token exposure
      </p>
      <div className="mt-3 space-y-2">
        {visible.length > 0 ? (
          visible.map((token) => (
            <ExposureRow
              key={token.symbol}
              token={token}
              maxWeight={maxWeight}
              netWorthUsd={netWorthUsd}
            />
          ))
        ) : (
          <EmptyLine>No token exposure yet</EmptyLine>
        )}
        {hidden > 0 && (
          <p className="text-[10px] text-text-mut">+{hidden} more tokens</p>
        )}
      </div>
    </section>
  );
}

function ExposureRow({
  token,
  maxWeight,
  netWorthUsd,
}: {
  token: DashboardTokenExposure;
  maxWeight: number;
  netWorthUsd: number;
}) {
  const width = Math.max(2, (token.weightPct / maxWeight) * 100);

  return (
    <div className="grid min-h-8 grid-cols-[minmax(80px,0.95fr)_minmax(70px,1.2fr)_minmax(70px,auto)_minmax(44px,auto)] items-center gap-2 text-[11px]">
      <div className="flex min-w-0 items-center gap-2">
        <TokenBadge symbol={token.symbol} className="h-5 w-5 shrink-0" />
        <span className="truncate font-semibold text-text-hi">
          {token.symbol}
        </span>
      </div>
      <div className="h-2 border border-border-default bg-bg">
        <div
          className="h-full bg-accent-pnl"
          style={{ width: `${width}%` }}
          aria-hidden
        />
      </div>
      <span className="text-right tabular-nums text-text-hi">
        {formatCurrency(token.totalUsd, { compact: true })}
      </span>
      <span className="text-right tabular-nums text-text-lo">
        {netWorthUsd > 0 ? `${token.weightPct.toFixed(1)}%` : "0%"}
      </span>
    </div>
  );
}

function ChainPanel({ chains }: { chains: DashboardChainExposure[] }) {
  const visible = chains.slice(0, 7);
  const hidden = Math.max(0, chains.length - visible.length);
  const maxWeight = Math.max(1, ...visible.map((chain) => chain.weightPct));

  return (
    <section className="min-w-0 border-b border-border-default p-4 md:border-b-0 xl:border-r">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        Chain distribution
      </p>
      <div className="mt-3 space-y-3">
        {visible.length > 0 ? (
          visible.map((chain) => (
            <ChainRow key={chain.key} chain={chain} maxWeight={maxWeight} />
          ))
        ) : (
          <EmptyLine>No wallet routes with value yet</EmptyLine>
        )}
        {hidden > 0 && (
          <p className="text-[10px] text-text-mut">+{hidden} more chains</p>
        )}
      </div>
    </section>
  );
}

function ChainRow({
  chain,
  maxWeight,
}: {
  chain: DashboardChainExposure;
  maxWeight: number;
}) {
  const width = Math.max(2, (chain.weightPct / maxWeight) * 100);

  return (
    <div className="grid min-h-10 grid-cols-[minmax(74px,0.8fr)_minmax(70px,1fr)_minmax(76px,auto)] items-center gap-2 text-[11px]">
      <span className="min-w-0 truncate font-semibold text-text-hi">
        {chain.shortLabel}
      </span>
      <div className="h-2 border border-border-default bg-bg">
        <div
          className="h-full bg-accent-pnl"
          style={{ width: `${width}%` }}
          aria-hidden
        />
      </div>
      <span className="min-w-0 text-right">
        <span className="block truncate tabular-nums text-text-hi">
          {formatCurrency(chain.totalUsd, { compact: true })}
        </span>
        <span className="block truncate tabular-nums text-[10px] text-text-lo">
          {chain.weightPct.toFixed(1)}%
        </span>
      </span>
    </div>
  );
}

function ActionPanel({
  model,
  executionProgress,
  onReviewPlan,
  reviewPlanLoading,
  onDesignAllocation,
  onOpenProposal,
  designLoading,
  designError,
  deployError,
  reviewMessage,
  proposalPending,
}: AssetControlTowerProps) {
  const action = actionState(model, proposalPending, executionProgress);
  const border =
    action.tone === "pnl"
      ? "border-accent-pnl/50 bg-accent-pnl/5"
      : action.tone === "agent"
        ? "border-accent-agent/50 bg-accent-agent/5"
        : action.tone === "warn"
          ? "border-warn/50 bg-warn/5"
          : "border-border-default bg-bg/45";

  return (
    <section className="order-first min-w-0 border-b border-t border-border-default p-4 md:col-span-2 xl:order-none xl:col-span-1 xl:border-t-0">
      <div className={cn("min-h-full border p-3", border)}>
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          Next action
        </p>
        <div className="mt-3 flex items-start gap-2">
          <ActionIcon tone={action.tone} />
          <div className="min-w-0">
            <p className={cn("text-sm font-semibold", toneClass(action.tone))}>
              {action.title}
            </p>
            <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
              {action.body}
            </p>
          </div>
        </div>

        <div className="mt-4 grid gap-2">
          {action.kind === "review" && (
            <BrutalButton
              type="button"
              variant="pnl"
              onClick={onReviewPlan}
              disabled={reviewPlanLoading}
              aria-busy={reviewPlanLoading}
              className="w-full"
            >
              {reviewPlanLoading ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Preparing review
                </>
              ) : (
                <>
                  Review plan
                  <ArrowRight className="h-4 w-4" />
                </>
              )}
            </BrutalButton>
          )}
          {action.kind === "design" && (
            <BrutalButton
              type="button"
              variant="agent"
              onClick={onDesignAllocation}
              disabled={designLoading}
              aria-busy={designLoading}
              className="w-full"
            >
              {designLoading ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Designing allocation
                </>
              ) : (
                <>
                  <Sparkles className="h-4 w-4" />
                  Design allocation
                </>
              )}
            </BrutalButton>
          )}
          {action.kind === "proposal" && (
            <BrutalButton
              type="button"
              variant="agent"
              onClick={onOpenProposal}
              className="w-full"
            >
              <Sparkles className="h-4 w-4" />
              Open Gate 1
              <ArrowRight className="h-4 w-4" />
            </BrutalButton>
          )}
          {action.kind === "fund" && <FaucetButton />}
          {action.kind === "wallet" && (
            <Link
              href="/wallets"
              className="inline-flex min-h-11 items-center justify-center gap-2 border border-warn/40 bg-warn/10 px-3 text-xs font-semibold text-warn hover:bg-warn/15"
            >
              Open wallet status
              <ArrowRight className="h-3.5 w-3.5" />
            </Link>
          )}
          {action.kind === "none" && (
            <Link
              href="/portfolio"
              className="inline-flex min-h-11 items-center justify-center gap-2 border border-border-default bg-bg/70 px-3 text-xs font-semibold text-text-hi hover:bg-raised"
            >
              Positions &amp; targets
              <ArrowRight className="h-3.5 w-3.5" />
            </Link>
          )}
          {action.kind === "execution" && executionProgress && (
            <>
              <ExecutionProgressMeter progress={executionProgress} />
              {executionProgress.status.toLowerCase() === "failed" && (
                <BrutalButton
                  type="button"
                  variant="pnl"
                  onClick={onReviewPlan}
                  disabled={reviewPlanLoading}
                  aria-busy={reviewPlanLoading}
                  className="w-full"
                >
                  {reviewPlanLoading ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Preparing review
                    </>
                  ) : (
                    <>
                      Build fresh review
                      <ArrowRight className="h-4 w-4" />
                    </>
                  )}
                </BrutalButton>
              )}
            </>
          )}
        </div>

        {designError && action.kind === "design" && (
          <AlertLine tone="risk">
            The agent could not finish designing. Try again.
          </AlertLine>
        )}
        {deployError &&
          (action.kind === "review" ||
            (action.kind === "execution" &&
              executionProgress?.status.toLowerCase() === "failed")) && (
            <AlertLine tone="risk">{deployError}</AlertLine>
          )}
        {reviewMessage &&
          (action.kind === "review" || action.kind === "execution") && (
            <AlertLine tone="agent">{reviewMessage}</AlertLine>
          )}
      </div>
    </section>
  );
}

function QuickStats({ model }: { model: DashboardBalanceModel }) {
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4">
      <QuickStat label="Tokens" value={String(model.tokenCount)} />
      <QuickStat label="Chains" value={String(model.chainCount)} />
      <QuickStat label="Addresses" value={String(model.addressCount)} />
      <QuickStat label="Last update" value={freshness(model)} />
    </div>
  );
}

function QuickStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-h-11 items-center gap-2 border-b border-r border-border-default px-4 py-2 [&:nth-child(2n)]:border-r-0 lg:[&:nth-child(2n)]:border-r lg:[&:nth-child(4n)]:border-r-0">
      <Activity className="h-3 w-3 text-text-mut" aria-hidden />
      <div className="flex min-w-0 items-baseline gap-2">
        <p className="min-w-0 truncate text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
        <p className="shrink-0 truncate text-[10px] font-semibold tabular-nums text-text-hi">
          {value}
        </p>
      </div>
    </div>
  );
}

function EmptyLine({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-10 border border-border-default bg-bg/45 px-3 py-2 text-[11px] text-text-lo">
      {children}
    </div>
  );
}

function AlertLine({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "risk" | "agent";
}) {
  return (
    <p
      className={cn(
        "mt-3 border px-2.5 py-2 text-[11px] leading-relaxed",
        tone === "risk"
          ? "border-risk/40 bg-risk/5 text-risk"
          : "border-accent-agent/40 bg-accent-agent/5 text-accent-agent",
      )}
      role={tone === "risk" ? "alert" : "status"}
    >
      {children}
    </p>
  );
}

function ActionIcon({ tone }: { tone: ActionTone }) {
  const className = cn("mt-0.5 h-4 w-4 shrink-0", toneClass(tone));
  if (tone === "agent") return <Sparkles className={className} />;
  if (tone === "pnl") return <ShieldCheck className={className} />;
  if (tone === "warn" || tone === "risk") {
    return <CircleAlert className={className} />;
  }
  return <Rocket className={className} />;
}

type ActionTone = "pnl" | "agent" | "warn" | "risk" | "muted";

interface TowerAction {
  kind:
    | "review"
    | "design"
    | "proposal"
    | "fund"
    | "wallet"
    | "execution"
    | "none";
  title: string;
  body: string;
  tone: ActionTone;
}

function actionState(
  model: DashboardBalanceModel,
  proposalPending: boolean,
  executionProgress?: ExecutionProgressSummary | null,
): TowerAction {
  const execution = executionAction(executionProgress);
  if (execution) return execution;

  if (model.walletBalanceUnavailable) {
    return {
      kind: "wallet",
      title: "Balance needs retry",
      body:
        model.gatewayBalanceError ??
        "Open wallet status and refresh the Circle balance check.",
      tone: "warn",
    };
  }
  if (model.walletBalanceLoading) {
    return {
      kind: "none",
      title: "Balance syncing",
      body: "Actions unlock when the current Circle balance settles.",
      tone: "agent",
    };
  }
  if (!model.hasIdleCash && !model.hasInvestedPositions) {
    return {
      kind: "fund",
      title: "Fund the wallet",
      body: "Add test USDC before Aegis can design or review a plan.",
      tone: "pnl",
    };
  }
  if (!model.hasAgentTarget && proposalPending) {
    return {
      kind: "proposal",
      title: "Allocation waiting for approval",
      body: "Open Gate 1 and approve the target allocation before funds move.",
      tone: "agent",
    };
  }
  if (!model.hasAgentTarget && model.hasIdleCash) {
    return {
      kind: "design",
      title: "Design the target mix",
      body: "Let the agent propose the allocation; you approve before execution.",
      tone: "agent",
    };
  }
  if (model.deployableUsd > 5 && model.hasAgentTarget) {
    return {
      kind: "review",
      title: "Review deployable surplus",
      body: `${formatCurrency(model.deployableUsd)} can be reviewed while ${formatCurrency(
        model.reserveUsd,
      )} remains reserve.`,
      tone: "pnl",
    };
  }
  if (model.hasInvestedPositions && model.hasReviewableDrift) {
    return {
      kind: "review",
      title: "Review target drift",
      body: `${model.maxTargetDriftPct.toFixed(1)}% drift is above the review threshold.`,
      tone: "warn",
    };
  }
  // Idle USDC fragmented across chains is consolidatable (CCTP, no price risk),
  // even when there's nothing else to trade. The predicate mirrors the backend
  // routing engine exactly (`idleUsdcConsolidation`), so the card appears
  // whenever the backend would actually plan a sweep — including a single
  // non-primary chain holding idle USDC, which the old "2+ funded chains"
  // heuristic hid.
  const consolidation = idleUsdcConsolidation(model.perChainUsdc);
  if (consolidation.sources >= 1) {
    const where =
      consolidation.fundedChains >= 2
        ? `Your USDC sits on ${consolidation.fundedChains} chains. `
        : "Some idle USDC is stranded off your main execution chain. ";
    return {
      kind: "review",
      title: "Consolidate idle USDC",
      body: `${where}Aegis can bridge it onto one chain over CCTP — no price risk — so it's ready to deploy. Review to approve.`,
      // Money accent: this is a cash-management step to review/approve (like the
      // deployable-surplus action), not agent activity — dual-accent rule.
      tone: "pnl",
    };
  }
  const trackedUsd = model.tokens
    .filter((token) => !isTradeableSleeve(token.symbol))
    .reduce((sum, token) => sum + token.totalUsd, 0);
  const trackedHeavy =
    model.netWorthUsd > 0 && trackedUsd / model.netWorthUsd > 0.5;
  if (trackedHeavy) {
    return {
      kind: "none",
      title: "Tracked, not traded here",
      body: "Most of your balance is volatile sleeves — tracked on this network, tradeable on mainnet. Aegis keeps your USDC managed and watches the market; there's no stablecoin move to make right now.",
      tone: "agent",
    };
  }
  return {
    kind: "none",
    title: "On target",
    body: "Aegis is monitoring balances, drift, and market conditions. Nothing to action right now.",
    tone: "muted",
  };
}

function executionAction(
  progress?: ExecutionProgressSummary | null,
): TowerAction | null {
  if (!progress) return null;
  const status = progress.status.toLowerCase();
  if (status === "completed") {
    return {
      kind: "execution",
      title: "Execution complete",
      body: `All ${progress.totalLegs} moves confirmed. Balances refresh from Circle after settlement.`,
      tone: "pnl",
    };
  }
  if (status === "failed") {
    return {
      kind: "execution",
      title: "Execution needs review",
      body:
        progress.failureReason ??
        "The executor stopped this plan before all moves confirmed.",
      tone: "risk",
    };
  }
  return {
    kind: "execution",
    title: "Execution in progress",
    body: `${progress.completedLegs} of ${progress.totalLegs} moves confirmed. Live route updates are running for this approved plan.`,
    tone: "agent",
  };
}

function executionStatusKpi(progress?: ExecutionProgressSummary | null): {
  label: string;
  detail: string;
  tone: "pnl" | "agent" | "risk";
} | null {
  if (!progress) return null;
  const status = progress.status.toLowerCase();
  if (status === "completed") {
    return {
      label: "Execution complete",
      detail: `${progress.totalLegs}/${progress.totalLegs} moves confirmed`,
      tone: "pnl",
    };
  }
  if (status === "failed") {
    return {
      label: "Execution failed",
      detail: progress.failureReason ?? "Review the latest route status.",
      tone: "risk",
    };
  }
  return {
    label: "Executing",
    detail: `${progress.completedLegs}/${progress.totalLegs} moves confirmed`,
    tone: "agent",
  };
}

function ExecutionProgressMeter({
  progress,
}: {
  progress: ExecutionProgressSummary;
}) {
  const total = Math.max(1, progress.totalLegs);
  const pct = Math.min(100, Math.round((progress.completedLegs / total) * 100));
  const status = progress.status.toLowerCase();
  const isActive = status !== "completed" && status !== "failed";
  return (
    <div
      className={cn(
        "border px-3 py-2",
        status === "failed"
          ? "border-risk/40 bg-risk/5"
          : status === "completed"
            ? "border-accent-pnl/40 bg-accent-pnl/5"
            : "border-accent-agent/40 bg-accent-agent/5",
      )}
      role="status"
      aria-live="polite"
    >
      <div className="flex items-center justify-between gap-3 text-[11px]">
        <span className="flex min-w-0 items-center gap-2 font-semibold text-text-hi">
          {isActive && (
            <Loader2
              className="h-3.5 w-3.5 shrink-0 animate-spin text-accent-agent"
              aria-hidden
            />
          )}
          <span className="truncate">{progress.status.toUpperCase()}</span>
        </span>
        <span className="shrink-0 tabular-nums text-text-lo">
          {progress.completedLegs}/{progress.totalLegs}
        </span>
      </div>
      <div className="mt-2 h-1.5 border border-border-default bg-bg">
        <div
          className={cn(
            "h-full transition-all duration-500",
            status === "failed"
              ? "bg-risk"
              : status === "completed"
                ? "bg-accent-pnl"
                : "bg-accent-agent",
          )}
          style={{ width: `${pct}%` }}
          aria-hidden
        />
      </div>
      <Link
        href={`/rebalance/${progress.rebalanceId}`}
        className="mt-2 inline-flex min-h-8 items-center gap-1.5 border border-border-default bg-bg/70 px-2.5 text-[10px] font-semibold text-text-hi hover:bg-raised"
      >
        Open trace
        <ArrowRight className="h-3 w-3" />
      </Link>
    </div>
  );
}

function toneClass(tone: "default" | ActionTone) {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "warn") return "text-warn";
  if (tone === "risk") return "text-risk";
  if (tone === "muted") return "text-text-lo";
  return "text-text-hi";
}

function freshness(model: DashboardBalanceModel) {
  if (model.walletBalanceUnavailable) return "needs retry";
  if (model.walletBalanceLoading) return "syncing";
  if (!model.gatewayBalanceUpdatedAt) return "live";
  return `refreshed ${timeAgo(new Date(model.gatewayBalanceUpdatedAt).toISOString())}`;
}
