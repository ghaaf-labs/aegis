"use client";

import Link from "next/link";
import {
  ArrowRight,
  BookOpen,
  CheckCircle2,
  CircleDollarSign,
  History,
  ReceiptText,
  ShieldAlert,
  ShieldCheck,
  SquareTerminal,
  Wallet,
  type LucideIcon,
} from "lucide-react";
import { usePortfolioStore } from "@/stores/portfolio";
import { cn } from "@/lib/utils";

interface HelpItem {
  href: string;
  icon: LucideIcon;
  label: string;
  title: string;
  body: string;
  action: string;
  evidence: string;
  tone: "pnl" | "agent" | "warn" | "risk";
  public?: boolean;
}

const HELP_ITEMS: HelpItem[] = [
  {
    href: "/wallets",
    icon: Wallet,
    label: "Wallet cash",
    title: "Wallet cash is $0 or unavailable",
    body: "Wallet cash is uninvested balance. If the cash check fails or returns zero, Aegis blocks review building instead of treating unknown cash as deployable.",
    action: "Refresh balances in Wallets",
    evidence: "Wallets -> Gateway balance -> per-chain tokens",
    tone: "pnl",
  },
  {
    href: "/transactions",
    icon: ShieldAlert,
    label: "Approval",
    title: "A review says needs changes",
    body: "A planned row can become stale when balances, routes, capabilities, or a newer review supersede it. Only the latest ready review can be approved.",
    action: "Open the latest review",
    evidence: "Transactions -> approval column -> open review",
    tone: "warn",
  },
  {
    href: "/transactions",
    icon: History,
    label: "Execution",
    title: "A route failed or stopped",
    body: "Failed rows remain visible for traceability. Open the trace to see the exact leg, then build a fresh review from current wallet and portfolio state.",
    action: "Open execution trace",
    evidence: "Transactions -> failed row -> open trace",
    tone: "risk",
  },
  {
    href: "/agent-logs",
    icon: SquareTerminal,
    label: "Agent reasoning",
    title: "What did the agent decide?",
    body: "Agent Logs shows the recommendation, model slug, confidence, critic verdict, evidence split, and whether the proposal is still current.",
    action: "Inspect agent logs",
    evidence: "Agent Logs -> current and history tabs",
    tone: "agent",
  },
  {
    href: "/settings",
    icon: BookOpen,
    label: "Diary privacy",
    title: "Public diary and leaderboard",
    body: "Diary sharing is opt-in. Public pages use an anonymous handle; private portfolios are excluded from diary lookup and leaderboard ranking.",
    action: "Review diary setting",
    evidence: "Settings -> public diary toggle",
    tone: "agent",
  },
  {
    href: "/tax-center",
    icon: ReceiptText,
    label: "Tax reports",
    title: "Tax exports and share links",
    body: "Tax Center exports settled activity only. Temporary accountant links are read-only and can be revoked from the same screen.",
    action: "Open tax center",
    evidence: "Tax Center -> download or share",
    tone: "agent",
  },
  {
    href: "/policy#refunds",
    icon: CircleDollarSign,
    label: "Fees",
    title: "Fees, gas, and refunds",
    body: "Every approval modal previews estimated USDC costs. Refund policy covers protocol-fee failures, not market movement after approval.",
    action: "Read refund policy",
    evidence: "Policy -> refunds",
    tone: "pnl",
    public: true,
  },
  {
    href: "/about/regime",
    icon: ShieldCheck,
    label: "Regime model",
    title: "Why risk-on, neutral, or risk-off?",
    body: "The classifier changes strategist posture. It does not execute by itself; it changes drift tolerance, cash posture, and review language.",
    action: "Open model card",
    evidence: "Regime model -> inputs and evidence",
    tone: "agent",
    public: true,
  },
];

export function HelpItemGrid() {
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);

  return (
    <section aria-label="Help topics" className="space-y-3">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
            Common situations
          </p>
          <h2 className="mt-1 font-mono text-lg font-semibold text-text-hi">
            Pick the state you are seeing
          </h2>
        </div>
        <span className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {sessionResolved && sessionActive
            ? "Account links active"
            : "Public-safe guide"}
        </span>
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        {HELP_ITEMS.map((item) => (
          <HelpCard
            key={`${item.href}-${item.title}`}
            item={item}
            sessionActive={sessionActive}
            sessionResolved={sessionResolved}
          />
        ))}
      </div>
    </section>
  );
}

function HelpCard({
  item,
  sessionActive,
  sessionResolved,
}: {
  item: HelpItem;
  sessionActive: boolean;
  sessionResolved: boolean;
}) {
  const locked = sessionResolved && !sessionActive && !item.public;
  const href = locked
    ? `/login?next=${encodeURIComponent(item.href)}`
    : item.href;
  const Icon = item.icon;

  return (
    <article className="border-brutal border-border-default bg-surface">
      <div className="grid gap-3 border-b border-border-default px-4 py-3 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-center">
        <span
          className={cn(
            "flex h-9 w-9 items-center justify-center border bg-bg",
            iconToneClass(item.tone),
          )}
        >
          <Icon className="h-4 w-4" aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <p className="font-mono text-sm font-semibold text-text-hi">
            {item.title}
          </p>
          <p className="mt-1 font-mono text-[10px] uppercase tracking-widest text-text-mut">
            {item.label}
          </p>
        </div>
        <AccessPill locked={locked} publicItem={Boolean(item.public)} />
      </div>
      <div className="space-y-3 px-4 py-4">
        <p className="text-sm leading-relaxed text-text-lo">{item.body}</p>
        <div className="grid gap-2 font-mono text-[11px] sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
          <div className="border border-border-default bg-bg px-3 py-2">
            <p className="text-[10px] uppercase tracking-widest text-text-mut">
              Where to verify
            </p>
            <p className="mt-1 text-text-hi">{item.evidence}</p>
          </div>
          <Link
            href={href}
            className={cn(
              "inline-flex min-h-10 items-center justify-center gap-2 border px-3 font-mono text-xs font-semibold",
              locked
                ? "border-accent-agent/40 bg-accent-agent/5 text-accent-agent hover:border-accent-agent"
                : "border-border-default bg-bg text-text-hi hover:border-accent-agent hover:text-accent-agent",
            )}
            aria-label={locked ? `Sign in to ${item.action}` : item.action}
          >
            {locked ? "Sign in" : item.action}
            <ArrowRight className="h-3.5 w-3.5" aria-hidden="true" />
          </Link>
        </div>
      </div>
    </article>
  );
}

function AccessPill({
  locked,
  publicItem,
}: {
  locked: boolean;
  publicItem: boolean;
}) {
  if (publicItem) {
    return (
      <span className="inline-flex min-h-7 items-center justify-center border border-accent-agent/35 bg-accent-agent/5 px-2 font-mono text-[10px] uppercase tracking-widest text-accent-agent">
        public
      </span>
    );
  }
  if (locked) {
    return (
      <span className="inline-flex min-h-7 items-center justify-center border border-border-default bg-bg px-2 font-mono text-[10px] uppercase tracking-widest text-text-mut">
        sign in
      </span>
    );
  }
  return (
    <span className="inline-flex min-h-7 items-center justify-center gap-1 border border-accent-pnl/35 bg-accent-pnl/5 px-2 font-mono text-[10px] uppercase tracking-widest text-accent-pnl">
      <CheckCircle2 className="h-3 w-3" aria-hidden="true" />
      open
    </span>
  );
}

function iconToneClass(tone: HelpItem["tone"]) {
  if (tone === "pnl") {
    return "border-accent-pnl/35 text-accent-pnl";
  }
  if (tone === "warn") {
    return "border-warn/40 text-warn";
  }
  if (tone === "risk") {
    return "border-risk/40 text-risk";
  }
  return "border-accent-agent/35 text-accent-agent";
}
