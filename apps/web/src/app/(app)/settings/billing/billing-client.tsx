"use client";

import { useEffect, useMemo, useState } from "react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
  ProvenanceLine,
} from "@aegis/ui";
import {
  DEFAULT_PRICING_TIERS,
  PricingTable,
} from "@/components/billing/PricingTable";
import { UpgradeModal } from "@/components/billing/UpgradeModal";
import { InvoiceList } from "@/components/billing/InvoiceList";
import { useBillingStore } from "@/stores/billing";
import { usePortfolioStore } from "@/stores/portfolio";
import { billingApi, type ReferralsResponse } from "@/lib/api";
import type { Tier } from "@/types";

function tierLabel(t: Tier): string {
  return t.charAt(0).toUpperCase() + t.slice(1);
}

export function BillingSettingsClient() {
  const subscription = useBillingStore((s) => s.subscription);
  const tiers = useBillingStore((s) => s.tiers);
  const invoices = useBillingStore((s) => s.invoices);
  const loading = useBillingStore((s) => s.loading);
  const error = useBillingStore((s) => s.error);
  const fetchBilling = useBillingStore((s) => s.fetch);
  const cancel = useBillingStore((s) => s.cancel);

  const portfolios = usePortfolioStore((s) => s.portfolios);
  const aumUsd = useMemo(
    () => portfolios.reduce((sum, p) => sum + (p.totalValueUsd ?? 0), 0),
    [portfolios],
  );

  const [pendingTier, setPendingTier] = useState<Tier | null>(null);
  const [busy, setBusy] = useState(false);
  const [alert, setAlert] = useState<{
    kind: "ok" | "err";
    msg: string;
  } | null>(null);

  const [referrals, setReferrals] = useState<ReferralsResponse | null>(null);

  useEffect(() => {
    void fetchBilling();
    billingApi
      .listReferrals()
      .then(setReferrals)
      .catch((e: unknown) => {
        if ((e as { status?: number })?.status !== 401)
          console.error("listReferrals failed", e);
      });
  }, [fetchBilling]);

  const effectiveTiers = tiers.length > 0 ? tiers : DEFAULT_PRICING_TIERS;
  const upgradesAvailable = tiers.length > 0;
  const currentTier: Tier = subscription?.tier ?? "free";
  const isCanceled = !!subscription?.cancelAt;

  const handleCancel = async () => {
    setBusy(true);
    setAlert(null);
    try {
      await cancel();
      setAlert({
        kind: "ok",
        msg: "Cancellation scheduled. You stay on your plan until the period ends.",
      });
    } catch (e) {
      setAlert({
        kind: "err",
        msg: e instanceof Error ? e.message : "cancel failed",
      });
    } finally {
      setBusy(false);
    }
  };

  const handleReenable = async () => {
    if (!subscription) return;
    setBusy(true);
    setAlert(null);
    try {
      const { billingApi } = await import("@/lib/api");
      const next = await billingApi.patchSubscription(subscription.id, {
        cancelAt: null,
      });
      useBillingStore.setState({ subscription: next });
      setAlert({ kind: "ok", msg: "Plan re-enabled." });
    } catch (e) {
      setAlert({
        kind: "err",
        msg: e instanceof Error ? e.message : "re-enable failed",
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-hi">Billing</h1>
          <p className="text-sm text-text-lo mt-1">
            Tiered SaaS + AUM streaming, settled in USDC on Arc.
          </p>
        </div>
      </header>

      {alert && (
        <div
          role={alert.kind === "err" ? "alert" : "status"}
          className={
            alert.kind === "ok"
              ? "border border-emerald-500/40 bg-emerald-500/10 p-3 text-xs text-accent-pnl"
              : "border border-red-500/40 bg-red-500/10 p-3 text-xs text-risk"
          }
        >
          {alert.msg}
        </div>
      )}

      {error && !alert && (
        <div className="border border-red-500/40 bg-red-500/10 p-3 text-xs text-risk">
          {error}
        </div>
      )}

      {!loading && !upgradesAvailable && (
        <div
          role="status"
          className="border border-border-default bg-surface/80 p-3 text-xs text-text-default"
        >
          Plan upgrades are not available right now. Your Free plan remains
          active.
        </div>
      )}

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-semibold text-text-hi">
            Current plan
          </span>
          <BrutalPill tone="pnl">{tierLabel(currentTier)}</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody>
          <dl className="grid grid-cols-2 md:grid-cols-4 gap-4 text-xs font-mono">
            <Stat
              label="Status"
              value={subscription?.status ?? (loading ? "loading…" : "free")}
            />
            <Stat
              label="Next billing"
              value={
                subscription
                  ? new Date(subscription.currentPeriodEnd).toLocaleDateString()
                  : "—"
              }
            />
            <Stat
              label="Portfolio AUM"
              value={`$${aumUsd.toLocaleString(undefined, { maximumFractionDigits: 0 })}`}
            />
            <Stat
              label="Models"
              value={
                effectiveTiers.find((t) => t.tier === currentTier)?.models ??
                "—"
              }
            />
          </dl>

          <div className="mt-5 flex flex-wrap gap-2">
            {upgradesAvailable && currentTier !== "business" && (
              <BrutalButton
                variant="pnl"
                onClick={() =>
                  setPendingTier(currentTier === "free" ? "pro" : "business")
                }
                aria-label="Upgrade plan"
              >
                Upgrade plan
              </BrutalButton>
            )}
            {subscription && !isCanceled && (
              <BrutalButton
                variant="ghost"
                onClick={handleCancel}
                disabled={busy}
              >
                Cancel at period end
              </BrutalButton>
            )}
            {isCanceled && subscription && (
              <div className="flex items-center gap-3 w-full md:w-auto border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-[11px] font-mono text-warn">
                <span>
                  Plan ends on{" "}
                  {new Date(
                    subscription.cancelAt as string,
                  ).toLocaleDateString()}
                </span>
                <BrutalButton
                  variant="pnl"
                  onClick={handleReenable}
                  disabled={busy}
                >
                  Re-enable
                </BrutalButton>
              </div>
            )}
          </div>

          <div className="mt-4">
            <ProvenanceLine source="USDC billing on Arc" />
          </div>
        </BrutalCardBody>
      </BrutalCard>

      <section>
        <h2 className="text-sm font-semibold text-text-hi mb-3">
          Compare plans
        </h2>
        <PricingTable
          tiers={effectiveTiers}
          currentTier={currentTier}
          actionsDisabled={!upgradesAvailable}
          disabledActionLabel="Unavailable"
          onSelect={(t) => {
            if (t !== currentTier) setPendingTier(t);
          }}
        />
      </section>

      <section>
        <InvoiceList invoices={invoices} />
      </section>

      {referrals && (
        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-semibold text-text-hi">
              Referral earnings
            </span>
            <BrutalPill tone="agent">
              {referrals.rows.length} referral
              {referrals.rows.length !== 1 ? "s" : ""}
            </BrutalPill>
          </BrutalCardHeader>
          <BrutalCardBody>
            <dl className="grid grid-cols-2 gap-4 text-xs font-mono mb-4">
              <Stat
                label="Paid out"
                value={`$${referrals.totalPaidUsdc.toFixed(2)} USDC`}
              />
              <Stat
                label="Pending"
                value={`$${referrals.totalPendingUsdc.toFixed(2)} USDC`}
              />
            </dl>
            {referrals.rows.length > 0 && (
              <div className="space-y-1 max-h-40 overflow-y-auto">
                {referrals.rows.slice(0, 10).map((r) => (
                  <div
                    key={r.id}
                    className="flex items-center justify-between text-[11px] font-mono text-text-lo border-b border-border-subtle pb-1"
                  >
                    <span>{new Date(r.createdAt).toLocaleDateString()}</span>
                    <span className={r.paidAt ? "text-pnl-green" : "text-warn"}>
                      {r.paidAt ? "paid" : "pending"} · $
                      {r.rewardUsdc.toFixed(2)}
                    </span>
                  </div>
                ))}
              </div>
            )}
            <div className="mt-3">
              <ProvenanceLine source="USDC referral rewards" />
            </div>
          </BrutalCardBody>
        </BrutalCard>
      )}

      {pendingTier && (
        <UpgradeModal
          open={true}
          targetTier={pendingTier}
          tiers={effectiveTiers}
          currentTier={currentTier}
          portfolioAumUsd={aumUsd}
          onClose={() => setPendingTier(null)}
          onUpgraded={() => {
            void fetchBilling();
            setAlert({
              kind: "ok",
              msg: `Upgraded to ${tierLabel(pendingTier)}.`,
            });
          }}
        />
      )}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-text-lo">{label}</dt>
      <dd className="text-text-hi mt-0.5 break-words">{value}</dd>
    </div>
  );
}
