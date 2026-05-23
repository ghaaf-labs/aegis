"use client";

import { ClipboardCheck, LineChart, Wallet } from "lucide-react";
import type { Portfolio } from "@/types";
import { formatCurrency } from "@/lib/utils";

interface ValueFlowCardProps {
  portfolio: Portfolio | null;
  idleUsdc: number;
  idleEurc: number;
  investedUsd: number;
  walletCashStatus?: "idle" | "loading" | "ready" | "error";
}

export function ValueFlowCard({
  portfolio,
  idleUsdc,
  idleEurc,
  investedUsd,
  walletCashStatus = "ready",
}: ValueFlowCardProps) {
  const walletCashKnown = walletCashStatus === "ready";
  const walletCashUnavailable = walletCashStatus === "error";
  const walletCashLoading =
    walletCashStatus === "idle" || walletCashStatus === "loading";
  const hasWalletCash = walletCashKnown && (idleUsdc > 0.5 || idleEurc > 0.5);
  const hasInvested = investedUsd > 0.5;
  const targetCount =
    portfolio?.allocations?.filter((a) => a.targetWeight > 0).length ?? 0;
  const walletCashText = walletCashUnavailable
    ? "Could not confirm wallet cash"
    : walletCashLoading
      ? "Checking wallet cash"
      : hasWalletCash
        ? `${formatCurrency(idleUsdc)} USDC${idleEurc > 0 ? ` + EURC ${idleEurc.toFixed(2)}` : ""}`
        : "No cash available";
  const reviewText = hasWalletCash
    ? "Ready to review"
    : targetCount > 0
      ? "No plan pending"
      : "Set a target first";
  const nextActionText = walletCashUnavailable
    ? "Retry wallet balance"
    : hasWalletCash
      ? "Review a plan"
      : targetCount > 0
        ? "Add funds"
        : "Create target";

  return (
    <section
      aria-label="Portfolio value status"
      className="rounded-sharp border-brutal border-border-default bg-bg p-4 md:p-5"
    >
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(280px,360px)] lg:items-start">
        <div className="font-mono">
          <p className="text-[10px] uppercase tracking-widest text-accent-agent">
            Money status
          </p>
          <h2 className="mt-1 text-lg font-semibold text-text-hi">
            Where your money is right now
          </h2>
          <p className="mt-2 max-w-2xl text-xs leading-relaxed text-text-lo">
            Cash is separate from invested positions. Aegis only changes that
            after you review and approve a plan.
          </p>
        </div>
        <div className="rounded-sharp border border-border-default bg-surface p-3 font-mono">
          <p className="text-[10px] uppercase tracking-widest text-text-mut">
            Next step
          </p>
          <p className="mt-1 text-sm font-semibold text-text-hi">
            {nextActionText}
          </p>
        </div>
      </div>

      <div className="mt-4 grid gap-2 md:grid-cols-3">
        <FlowStep
          icon={Wallet}
          label="1 Wallet cash"
          title={walletCashText}
          tone={
            walletCashUnavailable ? "warn" : hasWalletCash ? "pnl" : "neutral"
          }
        />
        <FlowStep
          icon={ClipboardCheck}
          label="2 Approval"
          title={reviewText}
          tone={hasWalletCash ? "agent" : "neutral"}
        />
        <FlowStep
          icon={LineChart}
          label="3 Invested"
          title={hasInvested ? formatCurrency(investedUsd) : "Not invested yet"}
          tone={hasInvested ? "pnl" : "neutral"}
        />
      </div>
    </section>
  );
}

function FlowStep({
  icon: Icon,
  label,
  title,
  tone,
}: {
  icon: typeof Wallet;
  label: string;
  title: string;
  tone: "pnl" | "agent" | "warn" | "neutral";
}) {
  const toneClass =
    tone === "pnl"
      ? "border-accent-pnl/45 bg-accent-pnl/5 text-accent-pnl"
      : tone === "agent"
        ? "border-accent-agent/45 bg-accent-agent/5 text-accent-agent"
        : tone === "warn"
          ? "border-warn/45 bg-warn/5 text-warn"
          : "border-border-default bg-surface text-text-lo";
  return (
    <div
      className={`flex min-h-16 items-center gap-3 rounded-sharp border px-3 py-2 font-mono ${toneClass}`}
    >
      <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-sharp border border-current bg-bg/70">
        <Icon className="h-4 w-4" />
      </span>
      <div className="min-w-0">
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
        <h3 className="mt-1 truncate text-sm font-semibold text-text-hi">
          {title}
        </h3>
      </div>
    </div>
  );
}
