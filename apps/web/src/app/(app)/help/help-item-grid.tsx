"use client";

import Link from "next/link";
import {
  ArrowRight,
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
  title: string;
  body: string;
  cta: string;
}

const HELP_ITEMS: HelpItem[] = [
  {
    href: "/wallets",
    icon: Wallet,
    title: "Why does wallet cash show $0?",
    body: "Wallets shows cash that is not invested yet. Dashboard and Portfolio show positions after an approved move finishes.",
    cta: "Open wallet cash view",
  },
  {
    href: "/transactions",
    icon: ShieldAlert,
    title: "Why is approval blocked?",
    body: "Old, failed, and completed plans stay visible for history, but only a fresh pending plan can run.",
    cta: "Open approval history",
  },
  {
    href: "/agent-logs",
    icon: LifeBuoy,
    title: "What did the agent decide?",
    body: "Agent Logs shows the recommendation, confidence, and safety notes behind each plan.",
    cta: "Open agent reasoning",
  },
  {
    href: "/tax-center",
    icon: ReceiptText,
    title: "How do tax exports work?",
    body: "Tax center downloads settled activity and creates temporary accountant links.",
    cta: "Open tax center",
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
    <div className="grid gap-3 md:grid-cols-2">
      {HELP_ITEMS.map((item) => {
        const href =
          sessionResolved && !sessionActive
            ? `/login?next=${encodeURIComponent(item.href)}`
            : item.href;
        const cta = sessionResolved && !sessionActive ? "Sign in" : item.cta;

        return (
          <Link key={item.href} href={href} className="group">
            <BrutalCard className="h-full group-hover:border-accent-agent/50">
              <BrutalCardHeader>
                <div className="flex items-center gap-2">
                  <item.icon className="h-4 w-4 text-accent-agent" />
                  <span className="text-sm font-mono text-text-hi">
                    {item.title}
                  </span>
                </div>
              </BrutalCardHeader>
              <BrutalCardBody className="space-y-3">
                <p className="text-sm font-mono leading-relaxed text-text-lo">
                  {item.body}
                </p>
                <div className="flex items-center justify-between gap-3">
                  <span className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
                    {accessLabel}
                  </span>
                  <span className="inline-flex items-center gap-1 font-mono text-[10px] uppercase tracking-widest text-accent-agent">
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
  );
}
