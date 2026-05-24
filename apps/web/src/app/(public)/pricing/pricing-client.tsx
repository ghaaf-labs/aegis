"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { Shield } from "lucide-react";
import {
  DEFAULT_PRICING_TIERS,
  PricingTable,
} from "@/components/billing/pricing-table";
import { UpgradeModal } from "@/components/billing/upgrade-modal";
import { useBillingStore } from "@/stores/billing";
import { walletApi } from "@/lib/api";
import { cn } from "@/lib/utils";
import { usePortfolioStore } from "@/stores/portfolio";
import type { PricingTier, Tier } from "@/types";

export function PricingPageClient() {
  const tiers = useBillingStore((s) => s.tiers);
  const subscription = useBillingStore((s) => s.subscription);
  const fetchBilling = useBillingStore((s) => s.fetch);
  const portfolios = usePortfolioStore((s) => s.portfolios);

  const [authed, setAuthed] = useState(false);
  const [pendingTier, setPendingTier] = useState<Tier | null>(null);

  useEffect(() => {
    let alive = true;
    walletApi
      .session()
      .then(() => {
        if (!alive) return;
        setAuthed(true);
        void fetchBilling();
      })
      .catch(() => {
        if (alive) setAuthed(false);
      });
    return () => {
      alive = false;
    };
  }, [fetchBilling]);

  const aumUsd = useMemo(
    () => portfolios.reduce((sum, p) => sum + (p.totalValueUsd ?? 0), 0),
    [portfolios],
  );

  const effectiveTiers = tiers.length > 0 ? tiers : DEFAULT_PRICING_TIERS;
  const currentTier: Tier = subscription?.tier ?? "free";
  const signupTone = "agent";

  return (
    <div className="min-h-screen bg-[#030712] text-text-hi">
      <nav className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-3 border-b border-white/5 px-4 py-4 sm:px-6 sm:py-5">
        <Link
          href="/"
          className="flex min-h-10 items-center gap-2 rounded-sharp"
        >
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center">
            <Shield className="w-4 h-4 text-text-hi" />
          </div>
          <span className="font-bold text-lg tracking-tight">Aegis</span>
        </Link>
        <div className="flex flex-wrap items-center justify-end gap-2 text-sm">
          <Link
            href="/pricing"
            className="inline-flex min-h-9 items-center rounded-sharp px-2 font-medium text-text-hi"
          >
            Pricing
          </Link>
          <Link
            href="/explore"
            className="inline-flex min-h-9 items-center rounded-sharp px-2 text-text-lo hover:text-text-hi"
          >
            Demo
          </Link>
          <Link href="/login" className={pricingLinkButtonClass(signupTone)}>
            Sign in
          </Link>
        </div>
      </nav>

      <section className="max-w-6xl mx-auto px-6 pt-16 pb-8 text-center">
        <h1 className="text-5xl font-bold tracking-tight mb-4">
          Stablecoin-native pricing
        </h1>
        <p className="text-lg text-text-lo max-w-2xl mx-auto">
          One portfolio per account. No hidden swap spread. No charging on
          failed execution.
        </p>
        <p className="mt-3 text-sm text-accent-agent/70 font-mono max-w-xl mx-auto">
          Paid billing via Circle Nanopayments is coming soon — USDC-native,
          monthly, with automatic refunds for failed legs.
        </p>
      </section>

      <section className="max-w-6xl mx-auto px-6 pb-16">
        {authed ? (
          <PricingTable
            tiers={effectiveTiers}
            currentTier={currentTier}
            onSelect={(tier) => {
              if (tier !== currentTier) setPendingTier(tier);
            }}
          />
        ) : (
          <PlanSelectGrid tiers={effectiveTiers} tone={signupTone} />
        )}
      </section>

      <section className="max-w-3xl mx-auto px-6 pb-16 text-center">
        <div className="border-2 border-white/10 bg-[#141414] p-8 shadow-[8px_8px_0_0_#000]">
          <h2 className="text-2xl font-bold mb-3">
            Free forever — upgrade only when it pays off.
          </h2>
          <p className="text-sm text-text-lo mb-4 max-w-xl mx-auto">
            Every approval modal shows the USDC fee upfront. You always see
            which model decided what and why.
          </p>
          {!authed && (
            <Link href="/login" className={pricingLinkButtonClass(signupTone)}>
              Start free
            </Link>
          )}
        </div>
      </section>

      <section className="max-w-3xl mx-auto px-6 pb-24">
        <details className="border border-white/10 bg-[#141414] group">
          <summary className="cursor-pointer px-5 py-4 text-sm font-mono text-text-lo hover:text-text-hi list-none flex items-center justify-between">
            <span>Fee mechanics &amp; technical details</span>
            <span className="font-mono text-[10px] text-text-mut group-open:hidden">
              ▶ show
            </span>
            <span className="font-mono text-[10px] text-text-mut hidden group-open:inline">
              ▼ hide
            </span>
          </summary>
          <div className="px-5 pb-5 space-y-3 text-xs font-mono text-text-lo border-t border-white/10 pt-4">
            <p>
              <span className="text-text-hi">Billing:</span> Circle Nanopayments
              (USDC on Base). Monthly subscription, settled per-leg. Coming soon
              — not live yet.
            </p>
            <p>
              <span className="text-text-hi">Gas:</span> Circle Paymaster covers
              gas in USDC on supported chains. Shown in the approval modal
              before execution.
            </p>
            <p>
              <span className="text-text-hi">Fee basis:</span> Charged as a flat
              fee per approved rebalance, not a percentage of AUM. Exact bps
              shown at approval time.
            </p>
            <p>
              <span className="text-text-hi">Models:</span> Strategist (Claude
              Opus), critic (GPT-5), regime classifier (Claude Haiku) — routed
              via OpenRouter per decision type.
            </p>
            <p>
              <span className="text-text-hi">Yield &amp; FX sleeves:</span> USYC
              and EURC allocations are coming soon — track-only in the current
              build.
            </p>
          </div>
        </details>
      </section>

      {pendingTier && (
        <UpgradeModal
          open={true}
          targetTier={pendingTier}
          tiers={effectiveTiers}
          currentTier={currentTier}
          portfolioAumUsd={aumUsd}
          onClose={() => setPendingTier(null)}
        />
      )}
    </div>
  );
}

function pricingLinkButtonClass(tone: "pnl" | "agent", className?: string) {
  return cn(
    "inline-flex min-h-10 items-center justify-center gap-2 px-3 py-2 text-sm font-semibold",
    "border-brutal border-black rounded-sharp text-black",
    tone === "agent" ? "bg-accent-agent" : "bg-accent-pnl",
    "transition-[box-shadow,transform] duration-100 hover:shadow-brutal-sm active:translate-y-px",
    className,
  );
}

function PlanSelectGrid({
  tiers,
  tone,
}: {
  tiers: PricingTier[];
  tone: "pnl" | "agent";
}) {
  return (
    <div className="grid gap-4 md:grid-cols-3" data-testid="pricing-table">
      {tiers.map((t) => {
        const href =
          t.tier === "free"
            ? "/login"
            : `/login?next=%2Fsettings%2Fbilling&plan=${t.tier}`;
        const isPro = t.tier === "pro" || t.recommended;
        return (
          <div
            key={t.tier}
            className={cn(
              "flex flex-col border-2 border-border-default bg-surface p-5",
              isPro && "shadow-brutal -translate-y-1 border-accent-agent/60",
            )}
            data-tier={t.tier}
          >
            <header className="flex items-center justify-between mb-3">
              <span className="border border-accent-agent/40 bg-accent-agent/5 px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest text-accent-agent">
                {(t.name ?? t.code).toUpperCase()}
              </span>
              {isPro && (
                <span className="border border-accent-pnl/40 bg-accent-pnl/5 px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest text-accent-pnl">
                  RECOMMENDED
                </span>
              )}
            </header>

            <div className="mb-4 font-mono">
              <span className="text-3xl font-bold text-text-hi tabular-nums">
                ${t.monthlyUsd}
              </span>
              <span className="text-text-lo text-sm">/mo</span>
            </div>

            <ul className="space-y-1.5 text-xs text-text-default mb-5 flex-1">
              {(t.features ?? []).map((f: string) => (
                <li key={f} className="flex items-start gap-2">
                  <span className="text-accent-pnl shrink-0">✓</span>
                  <span className="leading-relaxed">{f}</span>
                </li>
              ))}
            </ul>

            <Link
              href={href}
              className={cn(
                "inline-flex w-full items-center justify-center gap-2 px-3 py-2 text-sm font-semibold",
                "border-brutal border-black rounded-sharp text-black",
                tone === "agent" ? "bg-accent-agent" : "bg-accent-pnl",
                "transition-[box-shadow,transform] duration-100 hover:shadow-brutal-sm active:translate-y-px",
              )}
              aria-label={
                t.tier === "free"
                  ? "Start with the free plan"
                  : `Choose the ${t.name} plan`
              }
            >
              {t.tier === "free" ? "Start free" : `Choose ${t.name}`}
            </Link>
            <p className="mt-2 text-[11px] font-mono leading-relaxed text-text-mut">
              One email code. Manage the plan after sign-in.
            </p>
          </div>
        );
      })}
    </div>
  );
}
