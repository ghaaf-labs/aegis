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
    body: "Wallets shows idle USDC/EURC only. Invested positions live on Dashboard and Portfolio.",
    cta: "Open wallet cash view",
  },
  {
    href: "/transactions",
    icon: ShieldAlert,
    title: "Why is approval blocked?",
    body: "Transactions keeps stale, failed, historical, and completed plans visible without letting old plans execute.",
    cta: "Open approval history",
  },
  {
    href: "/agent-logs",
    icon: LifeBuoy,
    title: "What did the agent decide?",
    body: "Agent Logs shows the model slug, confidence, critic verdict, and recommendation summary.",
    cta: "Open agent reasoning",
  },
  {
    href: "/tax-center",
    icon: ReceiptText,
    title: "How do tax exports work?",
    body: "Tax center exports settled transaction rows and signed accountant links with clear caveats.",
    cta: "Open tax center",
  },
];

export function HelpItemGrid() {
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);

  const accessLabel = !sessionResolved
    ? "Checking access…"
    : sessionActive
      ? "In your account"
      : "Sign in to open";

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {HELP_ITEMS.map((item) => (
        <Link key={item.href} href={item.href} className="group">
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
                  {item.cta}
                  <ArrowRight className="h-3 w-3" aria-hidden="true" />
                </span>
              </div>
            </BrutalCardBody>
          </BrutalCard>
        </Link>
      ))}
    </div>
  );
}
