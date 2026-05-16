"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { Shield } from "lucide-react";
import { BrutalButton } from "@aegis/ui";
import {
  DEFAULT_PRICING_TIERS,
  PricingTable,
} from "@/components/billing/PricingTable";
import { UpgradeModal } from "@/components/billing/UpgradeModal";
import { useBillingStore } from "@/stores/billing";
import { getToken } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";
import type { Tier } from "@/types";

/** Cheap auth check — server-side cookies aren't exposed here, but the
 * legacy JWT path stores in localStorage. Treat either as "authenticated"
 * so the inline upgrade flow is offered. */
function hasSession(): boolean {
  if (typeof window === "undefined") return false;
  if (getToken()) return true;
  return document.cookie.includes("aegis_session=");
}

export function PricingPageClient() {
  const tiers = useBillingStore((s) => s.tiers);
  const subscription = useBillingStore((s) => s.subscription);
  const fetchBilling = useBillingStore((s) => s.fetch);
  const portfolios = usePortfolioStore((s) => s.portfolios);

  const [authed, setAuthed] = useState(false);
  const [pendingTier, setPendingTier] = useState<Tier | null>(null);

  useEffect(() => {
    setAuthed(hasSession());
    if (hasSession()) {
      void fetchBilling();
    }
  }, [fetchBilling]);

  const aumUsd = useMemo(
    () => portfolios.reduce((sum, p) => sum + (p.totalValueUsd ?? 0), 0),
    [portfolios],
  );

  const effectiveTiers = tiers.length > 0 ? tiers : DEFAULT_PRICING_TIERS;
  const currentTier: Tier = subscription?.tier ?? "free";

  return (
    <div className="min-h-screen bg-[#030712] text-white">
      <nav className="flex items-center justify-between px-6 py-5 max-w-7xl mx-auto border-b border-white/5">
        <Link href="/" className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center">
            <Shield className="w-4 h-4 text-white" />
          </div>
          <span className="font-bold text-lg tracking-tight">Aegis</span>
        </Link>
        <div className="flex items-center gap-3 text-sm">
          <Link href="/pricing" className="text-white font-medium">
            Pricing
          </Link>
          <Link href="/explore" className="text-gray-400 hover:text-white">
            Demo
          </Link>
          <Link href="/signup">
            <BrutalButton variant="pnl">Get started — free</BrutalButton>
          </Link>
        </div>
      </nav>

      <section className="max-w-6xl mx-auto px-6 pt-16 pb-12 text-center">
        <h1 className="text-5xl font-bold tracking-tight mb-4">
          Stablecoin-native pricing
        </h1>
        <p className="text-lg text-gray-400 max-w-2xl mx-auto">
          Pay in USDC. Billed monthly via Circle Nanopayments. No hidden swap
          spread. No charging on failed execution.
        </p>
      </section>

      <section className="max-w-6xl mx-auto px-6 pb-16">
        <PricingTable
          tiers={effectiveTiers}
          currentTier={authed ? currentTier : null}
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
          <p className="text-sm text-gray-400 mb-6 max-w-xl mx-auto">
            Every approval modal shows the USDC fee upfront. Failed legs are
            refunded automatically. You always see which model decided what.
          </p>
          {!authed && (
            <Link href="/signup">
              <BrutalButton variant="pnl">Get started — free</BrutalButton>
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
