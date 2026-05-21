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
} from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { DigestOptIn } from "@/components/settings/digest-opt-in";
import { DiaryVisibilityToggle } from "@/components/settings/diary-visibility-toggle";
import { portfolioApi } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";

interface SectionLink {
  href: string;
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  enabled?: boolean;
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
    setStoredEmail(localStorage.getItem("aegis_email") ?? "");
  }, []);

  const sections: SectionLink[] = [
    {
      href: "/wallets",
      icon: Wallet,
      title: "Wallets",
      description: "Per-chain USDC + EURC balances and addresses",
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

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3">
          Sections
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {sections
            .filter((s) => s.enabled !== false)
            .map((s) => (
              <Link
                key={s.href}
                href={s.href}
                className="group border-brutal border-border-default rounded-sharp bg-bg hover:border-border-hi p-4 flex items-start gap-3 transition-colors"
              >
                <s.icon className="w-4 h-4 text-accent-agent shrink-0 mt-0.5" />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-semibold text-text-hi font-mono">
                    {s.title}
                  </p>
                  <p className="text-[11px] text-text-lo font-mono mt-0.5 leading-relaxed">
                    {s.description}
                  </p>
                </div>
                <ArrowRight className="w-3.5 h-3.5 text-text-mut group-hover:text-text-hi shrink-0 mt-0.5" />
              </Link>
            ))}
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
