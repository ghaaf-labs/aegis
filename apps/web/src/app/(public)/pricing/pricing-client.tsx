"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { CircleAlert, Shield } from "lucide-react";
import {
  DEFAULT_PRICING_TIERS,
  PricingTable,
} from "@/components/billing/PricingTable";
import { UpgradeModal } from "@/components/billing/UpgradeModal";
import { useBillingStore } from "@/stores/billing";
import { walletApi, type WalletAuthReadinessResponse } from "@/lib/api";
import { cn } from "@/lib/utils";
import { usePortfolioStore } from "@/stores/portfolio";
import type { Tier } from "@/types";

export function PricingPageClient() {
  const tiers = useBillingStore((s) => s.tiers);
  const subscription = useBillingStore((s) => s.subscription);
  const fetchBilling = useBillingStore((s) => s.fetch);
  const portfolios = usePortfolioStore((s) => s.portfolios);

  const [authed, setAuthed] = useState(false);
  const [authReadiness, setAuthReadiness] =
    useState<WalletAuthReadinessResponse | null>(null);
  const [pendingTier, setPendingTier] = useState<Tier | null>(null);

  useEffect(() => {
    let alive = true;
    walletApi
      .me()
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

  useEffect(() => {
    let alive = true;
    walletApi
      .readiness()
      .then((readiness) => {
        if (alive) setAuthReadiness(readiness);
      })
      .catch(() => {
        if (alive) setAuthReadiness(null);
      });
    return () => {
      alive = false;
    };
  }, []);

  const aumUsd = useMemo(
    () => portfolios.reduce((sum, p) => sum + (p.totalValueUsd ?? 0), 0),
    [portfolios],
  );

  const effectiveTiers = tiers.length > 0 ? tiers : DEFAULT_PRICING_TIERS;
  const currentTier: Tier = subscription?.tier ?? "free";
  const authLocked =
    !!authReadiness &&
    !authReadiness.emailDeliveryConfigured &&
    !authReadiness.devCodesEnabled;
  const signupHref = authLocked ? "/signup?next=%2Fpricing" : "/signup";
  const signupLabel = authLocked ? "Open signup status" : "Get started — free";
  const signupTone = authLocked ? "agent" : "pnl";

  return (
    <div className="min-h-screen bg-[#030712] text-text-hi">
      <nav className="flex items-center justify-between px-6 py-5 max-w-7xl mx-auto border-b border-white/5">
        <Link href="/" className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center">
            <Shield className="w-4 h-4 text-text-hi" />
          </div>
          <span className="font-bold text-lg tracking-tight">Aegis</span>
        </Link>
        <div className="flex items-center gap-3 text-sm">
          <Link href="/pricing" className="text-text-hi font-medium">
            Pricing
          </Link>
          <Link href="/explore" className="text-text-lo hover:text-text-hi">
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

      {authLocked && (
        <section className="max-w-6xl mx-auto px-6 pb-8">
          <div className="grid gap-3 border-2 border-warn/40 bg-warn/5 p-4 font-mono md:grid-cols-[auto_1fr] md:items-start">
            <div className="flex h-9 w-9 items-center justify-center rounded-sharp border-brutal border-black bg-warn">
              <CircleAlert className="h-4 w-4 text-black" />
            </div>
            <div>
              <p className="text-[10px] uppercase tracking-widest text-warn">
                Real signup locked
              </p>
              <p className="mt-1 text-xs leading-relaxed text-text-lo">
                Pricing is visible, but this backend cannot send one-time
                verification codes yet. Aegis will not create a wallet or start
                paid billing from a remembered email alone.
              </p>
            </div>
          </div>
        </section>
      )}

      <section className="max-w-6xl mx-auto px-6 pb-16">
        <PricingTable
          tiers={effectiveTiers}
          currentTier={authed ? currentTier : null}
          publicActionHref={signupHref}
          publicActionLabel={authLocked ? "Open signup status" : undefined}
          publicActionTone={signupTone}
          publicActionHint={
            authLocked
              ? "Signup is waiting on email delivery. You can inspect the status, but no wallet or subscription is created."
              : null
          }
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
    "inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold",
    "border-brutal border-black rounded-sharp text-black",
    tone === "agent" ? "bg-accent-agent" : "bg-accent-pnl",
    "transition-[box-shadow,transform] duration-100 hover:shadow-brutal-sm active:translate-y-px",
    className,
  );
}
