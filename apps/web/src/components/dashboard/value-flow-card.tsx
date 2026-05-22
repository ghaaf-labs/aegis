"use client";

import { ArrowRight, ClipboardCheck, LineChart, Wallet } from "lucide-react";
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
  const hasIdleCash = walletCashKnown && (idleUsdc > 0.5 || idleEurc > 0.5);
  const hasInvested = investedUsd > 0.5;
  const targetCount =
    portfolio?.allocations?.filter((a) => a.targetWeight > 0).length ?? 0;
  const walletCashText = walletCashUnavailable
    ? "Could not confirm wallet cash"
    : walletCashLoading
      ? "Checking wallet cash"
      : hasIdleCash
        ? `${formatCurrency(idleUsdc)} USDC${idleEurc > 0 ? ` + EURC ${idleEurc.toFixed(2)}` : ""}`
        : "No cash ready";
  const reviewText = hasIdleCash
    ? "Review the next move"
    : targetCount > 0
      ? `${targetCount} target assets are set`
      : "Choose a target mix first";

  return (
    <section
      aria-label="How wallet cash becomes portfolio value"
      className="rounded-sharp border-brutal border-border-default bg-bg p-4 md:p-5"
    >
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-start">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
            How money moves
          </p>
          <h2 className="mt-1 font-mono text-lg font-semibold text-text-hi">
            Cash and investments are tracked separately
          </h2>
          <p className="mt-2 max-w-3xl font-mono text-xs leading-relaxed text-text-lo">
            Wallet cash is money waiting for your decision. Invested value only
            changes after an approved action finishes.
          </p>
        </div>
        <div className="grid min-w-[220px] gap-2 text-[11px] font-mono">
          <SummaryFact
            label="Wallet cash"
            value={walletCashText}
            tone={
              walletCashUnavailable ? "warn" : hasIdleCash ? "pnl" : "muted"
            }
          />
          <SummaryFact
            label="Invested value"
            value={formatCurrency(investedUsd)}
            tone={hasInvested ? "pnl" : "muted"}
          />
        </div>
      </div>

      <div className="mt-4 grid gap-3 lg:grid-cols-[1fr_auto_1fr_auto_1fr] lg:items-stretch">
        <FlowStep
          icon={Wallet}
          label="1. Wallet"
          title={walletCashText}
          copy={
            walletCashUnavailable
              ? "Aegis will not treat an unavailable balance as zero."
              : hasIdleCash
                ? "This is available cash, not invested value."
                : "Add test USDC before the first move."
          }
          tone={
            walletCashUnavailable ? "warn" : hasIdleCash ? "pnl" : "neutral"
          }
        />
        <FlowArrow />
        <FlowStep
          icon={ClipboardCheck}
          label="2. Review"
          title={reviewText}
          copy="You see the exact changes first. Nothing moves without your approval."
          tone={hasIdleCash ? "agent" : "neutral"}
        />
        <FlowArrow />
        <FlowStep
          icon={LineChart}
          label="3. Invested"
          title={hasInvested ? formatCurrency(investedUsd) : "Not invested yet"}
          copy={
            hasInvested
              ? "Only completed moves count here."
              : "This stays zero until an approved move finishes."
          }
          tone={hasInvested ? "pnl" : "neutral"}
        />
      </div>
    </section>
  );
}

function SummaryFact({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "pnl" | "warn" | "muted";
}) {
  return (
    <div className="rounded-sharp border border-border-default bg-surface px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={`mt-1 truncate font-semibold tabular-nums ${
          tone === "pnl"
            ? "text-accent-pnl"
            : tone === "warn"
              ? "text-warn"
              : "text-text-lo"
        }`}
      >
        {value}
      </p>
    </div>
  );
}

function FlowStep({
  icon: Icon,
  label,
  title,
  copy,
  tone,
}: {
  icon: typeof Wallet;
  label: string;
  title: string;
  copy: string;
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
    <div className={`rounded-sharp border p-4 font-mono ${toneClass}`}>
      <div className="flex items-center gap-2">
        <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-sharp border border-current bg-bg/70">
          <Icon className="h-4 w-4" />
        </span>
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <h3 className="mt-3 text-sm font-semibold text-text-hi">{title}</h3>
      <p className="mt-2 text-xs leading-relaxed text-text-lo">{copy}</p>
    </div>
  );
}

function FlowArrow() {
  return (
    <div className="hidden items-center justify-center text-text-mut lg:flex">
      <ArrowRight className="h-5 w-5" />
    </div>
  );
}
