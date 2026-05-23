"use client";

import Link from "next/link";
import {
  ArrowRight,
  CheckCircle2,
  LifeBuoy,
  ReceiptText,
  ShieldAlert,
  Wallet,
  type LucideIcon,
} from "lucide-react";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";
import { usePortfolioStore } from "@/stores/portfolio";

interface HelpItem {
  href: string;
  icon: LucideIcon;
  label: string;
  title: string;
  body: string;
  cta: string;
  tone: "pnl" | "agent" | "warn";
}

const HELP_ITEMS: HelpItem[] = [
  {
    href: "/wallets",
    icon: Wallet,
    label: "Wallet cash",
    title: "Why does wallet cash show $0?",
    body: "Wallets shows cash that is not invested yet. Dashboard and Portfolio show positions after an approved move finishes.",
    cta: "Open wallet cash view",
    tone: "pnl",
  },
  {
    href: "/transactions",
    icon: ShieldAlert,
    label: "Approval",
    title: "Why does approval need changes?",
    body: "Old, failed, and completed plans stay visible for history, but only a fresh ready review can run.",
    cta: "Open approval history",
    tone: "warn",
  },
  {
    href: "/agent-logs",
    icon: LifeBuoy,
    label: "Agent reasoning",
    title: "What did the agent decide?",
    body: "Agent Logs shows the recommendation, confidence, and safety notes behind each plan.",
    cta: "Open agent reasoning",
    tone: "agent",
  },
  {
    href: "/tax-center",
    icon: ReceiptText,
    label: "Reports",
    title: "How do tax exports work?",
    body: "Tax center downloads settled activity and creates temporary accountant links.",
    cta: "Open tax center",
    tone: "agent",
  },
];

export function HelpItemGrid() {
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);

  const accessLabel = !sessionResolved
    ? "Account page"
    : sessionActive
      ? "Ready to open"
      : "Sign in to open";

  return (
    <section aria-label="Help topics" className="space-y-3">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
            Common questions
          </p>
          <h2 className="mt-1 font-mono text-lg font-semibold text-text-hi">
            Fix the exact thing you are confused by
          </h2>
        </div>
        <span className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {sessionResolved && sessionActive
            ? "Account ready"
            : "Signed-out safe"}
        </span>
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        {HELP_ITEMS.map((item) => {
          const href =
            sessionResolved && !sessionActive
              ? `/login?next=${encodeURIComponent(item.href)}`
              : item.href;
          const cta = sessionResolved && !sessionActive ? "Sign in" : item.cta;
          const toneClass = itemToneClass(item.tone);

          return (
            <Link key={item.href} href={href} className="group">
              <BrutalCard className="h-full transition-transform group-hover:-translate-x-0.5 group-hover:-translate-y-0.5 group-hover:border-accent-agent/50 group-hover:shadow-brutal">
                <BrutalCardHeader className="gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <span
                      className={`flex h-8 w-8 shrink-0 items-center justify-center border ${toneClass}`}
                    >
                      <item.icon className="h-4 w-4" />
                    </span>
                    <span className="min-w-0 text-sm font-mono text-text-hi">
                      {item.title}
                    </span>
                  </div>
                  <span className="shrink-0 border border-border-default bg-bg px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wider text-text-mut">
                    {item.label}
                  </span>
                </BrutalCardHeader>
                <BrutalCardBody className="space-y-3">
                  <p className="text-sm font-mono leading-relaxed text-text-lo">
                    {item.body}
                  </p>
                  <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
                    <span className="inline-flex min-h-8 items-center gap-1.5 border border-border-default bg-bg px-2 font-mono text-[10px] uppercase tracking-widest text-text-mut">
                      {sessionResolved && sessionActive && (
                        <CheckCircle2 className="h-3 w-3 text-accent-pnl" />
                      )}
                      {accessLabel}
                    </span>
                    <span className="inline-flex min-h-8 items-center justify-center gap-1 border border-accent-agent/35 bg-accent-agent/5 px-2 font-mono text-[10px] uppercase tracking-widest text-accent-agent group-hover:border-accent-agent">
                      {cta}
                      <ArrowRight className="h-3 w-3" aria-hidden="true" />
                    </span>
                  </div>
                </BrutalCardBody>
              </BrutalCard>
            </Link>
          );
        })}
      </div>
    </section>
  );
}

function itemToneClass(tone: HelpItem["tone"]) {
  if (tone === "pnl") {
    return "border-accent-pnl/35 bg-accent-pnl/5 text-accent-pnl";
  }
  if (tone === "warn") {
    return "border-warn/40 bg-warn/5 text-warn";
  }
  return "border-accent-agent/35 bg-accent-agent/5 text-accent-agent";
}
