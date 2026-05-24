"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { ArrowRight, CreditCard } from "lucide-react";
import { ProvenanceLine } from "@aegis/ui";
import { DEFAULT_PRICING_TIERS } from "@/components/billing/pricing-table";
import { agentApi, billingApi } from "@/lib/api";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import type { Subscription, Tier } from "@/types";

const TIER_LABEL: Record<Tier, string> = {
  free: "Free",
  pro: "Pro",
  business: "Business",
};

// `GET /billing/subscription` returns `{ subscription, implicit }` when
// billing v2 is on; the shared client types it loosely as `Subscription |
// null`. Accept either shape so a real subscription's `tier` is read
// correctly.
function unwrapSubscription(value: Subscription | null): Subscription | null {
  if (value && typeof value === "object" && "subscription" in value) {
    return (value as { subscription: Subscription }).subscription;
  }
  return value;
}

function startOfBillingPeriod(sub: Subscription | null): Date {
  if (sub?.currentPeriodStart) {
    const d = new Date(sub.currentPeriodStart);
    if (!Number.isNaN(d.getTime())) return d;
  }
  // Free users have no subscription row — fall back to a calendar month.
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), 1);
}

/**
 * Always-visible plan summary on the settings index. Tier comes from the real
 * subscription (`GET /billing/subscription`; `null` => Free, enforced
 * server-side). Usage counts the user's own agent decisions in the current
 * billing period against the tier's monthly cap — no invented numbers. Full
 * tier comparison / upgrade lives behind the pricing-UI flag at
 * `/settings/billing`; otherwise we show an honest "opening soon" note rather
 * than a fake checkout.
 */
export function PlanUsage({ portfolioId }: { portfolioId: string }) {
  const [subscription, setSubscription] = useState<Subscription | null>(null);
  const [decisionDates, setDecisionDates] = useState<string[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    billingApi
      .getSubscription()
      .then((s) => {
        if (!cancelled) setSubscription(unwrapSubscription(s));
      })
      .catch(() => {
        // 404 when BILLING_V2_ENABLED=false (the committed default) — Free.
        if (!cancelled) setSubscription(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!portfolioId) {
      setDecisionDates([]);
      return;
    }
    let cancelled = false;
    agentApi
      .decisions(portfolioId)
      .then((rows) => {
        if (!cancelled) setDecisionDates(rows.map((r) => r.createdAt));
      })
      .catch(() => {
        if (!cancelled) setDecisionDates(null);
      });
    return () => {
      cancelled = true;
    };
  }, [portfolioId]);

  const tier: Tier = subscription?.tier ?? "free";
  const tierMeta = DEFAULT_PRICING_TIERS.find((t) => t.code === tier);
  const monthlyCap = tierMeta?.decisionsCapMonthly ?? null;

  const usedThisPeriod = useMemo(() => {
    if (!decisionDates) return null;
    const since = startOfBillingPeriod(subscription).getTime();
    return decisionDates.filter((d) => {
      const t = new Date(d).getTime();
      return !Number.isNaN(t) && t >= since;
    }).length;
  }, [decisionDates, subscription]);

  const capLabel = monthlyCap === null ? "unlimited" : `${monthlyCap} / month`;
  const usageLabel =
    usedThisPeriod === null
      ? "—"
      : monthlyCap === null
        ? `${usedThisPeriod} this period`
        : `${usedThisPeriod} of ${monthlyCap}`;

  return (
    <div className="rounded-sharp border-brutal border-border-default bg-bg p-4">
      <div className="flex items-start gap-3">
        <CreditCard className="mt-0.5 h-4 w-4 shrink-0 text-accent-pnl" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <p className="font-mono text-sm font-semibold text-text-hi">
              Plan & usage
            </p>
            <span className="rounded-sharp border-brutal border-accent-pnl/40 bg-accent-pnl/10 px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest text-accent-pnl">
              {TIER_LABEL[tier]}
            </span>
          </div>

          <dl className="mt-3 grid grid-cols-2 gap-4 font-mono text-[11px]">
            <div>
              <dt className="text-text-lo">Monthly price</dt>
              <dd className="mt-0.5 text-text-hi">
                {tierMeta ? `$${tierMeta.monthlyUsd}` : "—"}
              </dd>
            </div>
            <div>
              <dt className="text-text-lo">Decision cap</dt>
              <dd className="mt-0.5 text-text-hi">{capLabel}</dd>
            </div>
            <div>
              <dt className="text-text-lo">Decisions used</dt>
              <dd className="mt-0.5 text-text-hi">{usageLabel}</dd>
            </div>
            <div>
              <dt className="text-text-lo">Renews</dt>
              <dd className="mt-0.5 text-text-hi">
                {subscription
                  ? new Date(subscription.currentPeriodEnd).toLocaleDateString()
                  : "—"}
              </dd>
            </div>
          </dl>

          {PRICING_UI_ENABLED ? (
            <Link
              href="/settings/billing"
              className="mt-4 inline-flex min-h-9 items-center gap-2 rounded-sharp border-brutal border-black bg-accent-pnl px-3 font-mono text-[12px] font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
            >
              {tier === "business" ? "Manage plan" : "Compare & upgrade"}
              <ArrowRight className="h-3.5 w-3.5" />
            </Link>
          ) : (
            <p className="mt-4 font-mono text-[11px] leading-relaxed text-text-mut">
              Plan changes open soon. Free stays active with no card on file.
            </p>
          )}

          <div className="mt-3">
            <ProvenanceLine source="USDC billing on Arc" />
          </div>
        </div>
      </div>
    </div>
  );
}
