"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { Shield } from "lucide-react";
import {
  DEFAULT_PRICING_TIERS,
  PricingTable,
} from "@/components/billing/PricingTable";
import { UpgradeModal } from "@/components/billing/UpgradeModal";
import { useBillingStore } from "@/stores/billing";
import { walletApi } from "@/lib/api";
import { cn } from "@/lib/utils";
import { usePortfolioStore } from "@/stores/portfolio";
import type { Tier } from "@/types";

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
  const signupHref = "/login?next=%2Fpricing";
  const signupLabel = "Continue";
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
          <Link
            href={signupHref}
            className={pricingLinkButtonClass(signupTone)}
          >
            {signupLabel}
          </Link>
        </div>
      </nav>

      <section className="max-w-6xl mx-auto px-6 pt-16 pb-12 text-center">
        <h1 className="text-5xl font-bold tracking-tight mb-4">
          Stablecoin-native pricing
        </h1>
        <p className="text-lg text-text-lo max-w-2xl mx-auto">
          Pay in USDC. Billed monthly via Circle Nanopayments. No hidden swap
          spread. No charging on failed execution.
        </p>
      </section>

      <section className="max-w-6xl mx-auto px-6 pb-16">
        <PricingTable
          tiers={effectiveTiers}
          currentTier={authed ? currentTier : null}
          publicActionHref={signupHref}
          publicActionLabel="Continue"
          publicActionTone={signupTone}
          publicActionHint={null}
          onSelect={
            authed
              ? (tier) => {
                  if (tier !== currentTier) setPendingTier(tier);
                }
              : undefined
          }
        />
      </section>

      <section className="max-w-3xl mx-auto px-6 pb-24 text-center">
        <div className="border-2 border-white/10 bg-[#141414] p-8 shadow-[8px_8px_0_0_#000]">
          <h2 className="text-2xl font-bold mb-3">
            Free forever — upgrade only when it pays off.
          </h2>
          <p className="text-sm text-text-lo mb-6 max-w-xl mx-auto">
            Every approval modal shows the USDC fee upfront. Failed legs are
            refunded automatically. You always see which model decided what.
          </p>
          {!authed && (
            <Link
              href={signupHref}
              className={pricingLinkButtonClass(signupTone)}
            >
              {signupLabel}
            </Link>
          )}
        </div>
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
