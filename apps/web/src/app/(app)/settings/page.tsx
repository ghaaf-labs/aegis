"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  BarChart3,
  Bot,
  CircleHelp,
  Wallet,
  Shield,
  Receipt,
  CreditCard,
  AlertTriangle,
  Mail,
  Eye,
  ArrowRight,
  ListChecks,
  SquareTerminal,
  LockKeyhole,
} from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { DigestOptIn } from "@/components/settings/digest-opt-in";
import { DiaryVisibilityToggle } from "@/components/settings/diary-visibility-toggle";
import { portfolioApi, walletApi } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";

interface SectionLink {
  href: string;
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  enabled?: boolean;
  requiresWallet?: boolean;
}

export default function SettingsIndex() {
  const portfolio = useActivePortfolio();
  const wallet = usePortfolioStore((s) => s.wallet);
  const portfolioId = portfolio?.id ?? "";

  const diaryQuery = useApiQuery(
    `portfolio.diaryPublic.${portfolioId}`,
    () => portfolioApi.getDiaryPublic(portfolioId),
    { enabled: !!portfolioId },
  );
  const [localDiaryPublic, setLocalDiaryPublic] = useState<boolean | null>(
    null,
  );
  const diaryPublic = localDiaryPublic ?? diaryQuery.data?.diaryPublic ?? false;

  const [storedEmail, setStoredEmail] = useState("");
  useEffect(() => {
    let cancelled = false;
    const remembered = localStorage.getItem("aegis_email") ?? "";
    setStoredEmail(remembered);
    if (remembered) return;
    walletApi
      .me()
      .then((user) => {
        if (cancelled) return;
        localStorage.setItem("aegis_email", user.email);
        setStoredEmail(user.email);
      })
      .catch(() => {
        if (!cancelled) setStoredEmail("");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const sections: SectionLink[] = [
    {
      href: "/wallets",
      icon: Wallet,
      title: "Wallets",
      description: "Per-chain USDC + EURC balances and addresses",
      requiresWallet: false,
    },
    {
      href: "/transactions",
      icon: ListChecks,
      title: "Transactions",
      description: "Rebalance plans, approval status, and execution history",
    },
    {
      href: "/analytics",
      icon: BarChart3,
      title: "Analytics",
      description: "Net worth, target allocation, regime, and confidence",
    },
    {
      href: "/settings/agent",
      icon: Shield,
      title: "Agent",
      description: "Pause / resume the agent, view trigger thresholds",
    },
    {
      href: "/agent-logs",
      icon: SquareTerminal,
      title: "Agent logs",
      description: "Model slugs, confidence, critic notes, and decisions",
    },
    {
      href: "/agent-studio",
      icon: Bot,
      title: "Agent Studio",
      description: "Manual analysis, pause controls, and agent inputs",
    },
    {
      href: "/settings/peg",
      icon: AlertTriangle,
      title: "Peg defense",
      description: "Stablecoin peg-monitor rules + thresholds",
    },
    {
      href: "/tax-center",
      icon: Receipt,
      title: "Tax center",
      description: "Portfolio-level FIFO CSVs + accountant share links",
    },
    {
      href: "/help",
      icon: CircleHelp,
      title: "Help",
      description: "Answers for wallet cash, approvals, logs, and exports",
      requiresWallet: false,
    },
    {
      href: "/settings/billing",
      icon: CreditCard,
      title: "Billing",
      description: "Subscription tier, fee history, payment method",
      enabled: PRICING_UI_ENABLED,
    },
  ];

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
        Settings
      </h1>

      {!wallet && (
        <section className="border border-warn/40 bg-warn/5 p-4 font-mono">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p className="text-[10px] uppercase tracking-widest text-warn">
                Wallet setup required
              </p>
              <p className="mt-2 max-w-2xl text-xs leading-relaxed text-text-lo">
                This browser may have an app session, but Aegis has not received
                real Arc + Base Circle wallet addresses yet. Portfolio, tax,
                billing, peg, and agent controls stay locked until wallet setup
                finishes.
              </p>
            </div>
            <Link
              href="/wallets"
              className="inline-flex min-h-9 items-center justify-center rounded-sharp border border-warn/40 bg-bg px-3 text-[11px] uppercase tracking-widest text-warn hover:bg-warn/10"
            >
              Finish setup
            </Link>
          </div>
        </section>
      )}

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3">
          Sections
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {sections
            .filter((s) => s.enabled !== false)
            .map((s) => {
              const locked = !wallet && s.requiresWallet !== false;
              return (
                <Link
                  key={s.href}
                  href={locked ? "/wallets" : s.href}
                  title={
                    locked
                      ? `${s.title} unlocks after Circle returns Arc + Base wallet addresses`
                      : s.title
                  }
                  className={`group flex items-start gap-3 rounded-sharp border-brutal bg-bg p-4 transition-colors ${
                    locked
                      ? "border-warn/35 hover:border-warn/60"
                      : "border-border-default hover:border-border-hi"
                  }`}
                >
                  <s.icon
                    className={`mt-0.5 h-4 w-4 shrink-0 ${
                      locked ? "text-warn" : "text-accent-agent"
                    }`}
                  />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-semibold text-text-hi font-mono">
                      {s.title}
                    </p>
                    <p className="text-[11px] text-text-lo font-mono mt-0.5 leading-relaxed">
                      {locked
                        ? "Finish wallet setup first. This page uses wallet-backed data or actions."
                        : s.description}
                    </p>
                  </div>
                  {locked ? (
                    <LockKeyhole className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warn" />
                  ) : (
                    <ArrowRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-text-mut group-hover:text-text-hi" />
                  )}
                </Link>
              );
            })}
        </div>
      </section>

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3 flex items-center gap-2">
          <Mail className="w-3 h-3" /> Notifications
        </h2>
        <DigestOptIn defaultEmail={storedEmail} />
      </section>

      {portfolioId && (
        <section>
          <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3 flex items-center gap-2">
            <Eye className="w-3 h-3" /> Privacy
          </h2>
          <DiaryVisibilityToggle
            key={`diary-${portfolioId}-${diaryPublic}`}
            initialPublic={diaryPublic}
            walletAddress={wallet?.arcAddress}
            onChange={async (next) => {
              const res = await portfolioApi.setDiaryPublic(portfolioId, next);
              setLocalDiaryPublic(res.diaryPublic);
            }}
          />
        </section>
      )}
    </div>
  );
}
