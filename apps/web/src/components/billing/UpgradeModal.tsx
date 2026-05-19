"use client";

import { useMemo, useState } from "react";
import { BrutalButton, BrutalPill, ProvenanceLine } from "@aegis/ui";
import type { PricingTier, Tier } from "@/types";
import { useBillingStore } from "@/stores/billing";

export interface UpgradeModalProps {
  open: boolean;
  /** Tier the user is upgrading TO. */
  targetTier: Tier;
  tiers: PricingTier[];
  /** User's current tier; used to compute the price delta. */
  currentTier: Tier;
  /** Live portfolio AUM (USD) — drives the AUM-fee delta calculation. */
  portfolioAumUsd: number;
  /** Counterfactual: USDC saved had the user been on `targetTier` last month.
   * Optional; when 0 or undefined, the line is hidden. */
  lastMonthSavingsUsd?: number;
  onClose: () => void;
  onUpgraded?: (tier: Tier) => void;
}

function tierBy(tiers: PricingTier[], slug: Tier): PricingTier | null {
  return tiers.find((t) => t.tier === slug) ?? null;
}

function nextMonthAnchor(): string {
  const d = new Date();
  d.setUTCMonth(d.getUTCMonth() + 1);
  return d.toISOString().slice(0, 10);
}

export function UpgradeModal({
  open,
  targetTier,
  tiers,
  currentTier,
  portfolioAumUsd,
  lastMonthSavingsUsd,
  onClose,
  onUpgraded,
}: UpgradeModalProps) {
  const upgrade = useBillingStore((s) => s.upgrade);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const current = useMemo(
    () => tierBy(tiers, currentTier),
    [tiers, currentTier],
  );
  const next = useMemo(() => tierBy(tiers, targetTier), [tiers, targetTier]);

  if (!open || !next) return null;

  const monthlyDeltaUsd = (next?.monthlyUsd ?? 0) - (current?.monthlyUsd ?? 0);
  const aumDeltaBps = (next?.aumAnnualBps ?? 0) - (current?.aumAnnualBps ?? 0);
  // Monthly AUM-fee delta in USDC: (AUM × bps / 10_000) / 12
  const aumFeeDeltaUsdc = (portfolioAumUsd * aumDeltaBps) / 10_000 / 12;

  const handleSubmit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await upgrade(targetTier);
      setSuccess(`You're now on ${next.name}. Welcome.`);
      onUpgraded?.(targetTier);
      // Auto-dismiss the alert so the dashboard refresh feels snappy.
      setTimeout(onClose, 1200);
    } catch (e) {
      setError(e instanceof Error ? e.message : "upgrade failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="upgrade-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
    >
      <div className="w-full max-w-lg bg-[#141414] border-2 border-white/15 shadow-[8px_8px_0_0_#000]">
        <header className="px-6 py-4 border-b border-white/10 flex items-center justify-between">
          <div>
            <h2
              id="upgrade-modal-title"
              className="text-base font-semibold text-text-hi"
            >
              Confirm upgrade
            </h2>
            <p className="text-[11px] font-mono text-text-lo mt-1 flex items-center gap-2">
              <BrutalPill tone="agent">{currentTier.toUpperCase()}</BrutalPill>
              <span className="text-text-mut">→</span>
              <BrutalPill tone="pnl">{targetTier.toUpperCase()}</BrutalPill>
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-text-lo hover:text-text-hi"
            aria-label="Close"
          >
            ×
          </button>
        </header>

        <div className="px-6 py-4 space-y-3 text-xs font-mono">
          <Row
            label="Monthly subscription"
            value={`+$${monthlyDeltaUsd.toFixed(2)} / mo`}
            valueClass="text-accent-pnl"
          />
          <Row
            label={`AUM fee (on $${portfolioAumUsd.toLocaleString(undefined, { maximumFractionDigits: 0 })})`}
            value={
              aumDeltaBps === 0
                ? "no change"
                : `${aumDeltaBps > 0 ? "+" : ""}$${aumFeeDeltaUsdc.toFixed(2)} / mo @ ${aumDeltaBps} bps`
            }
            valueClass={
              aumDeltaBps > 0 ? "text-accent-pnl" : "text-text-default"
            }
          />
          <Row
            label="Per-rebalance fee"
            value={`${next.perRebalanceBps} bps`}
            valueClass="text-text-hi"
          />
          <Row
            label="First billing date"
            value={nextMonthAnchor()}
            valueClass="text-text-hi"
          />

          {lastMonthSavingsUsd !== undefined && lastMonthSavingsUsd > 0 && (
            <div className="mt-3 border border-emerald-500/30 bg-emerald-500/5 p-3 text-[11px] text-accent-pnl leading-relaxed">
              At {next.name} you would have saved{" "}
              <span className="font-bold">
                ${lastMonthSavingsUsd.toFixed(2)}
              </span>{" "}
              in rebalance fees last month.
            </div>
          )}

          <div className="pt-3 border-t border-white/5">
            <ProvenanceLine source="Circle Nanopayments · USDC on Arc" />
          </div>

          {error && (
            <div
              role="alert"
              className="border border-red-500/40 bg-red-500/10 p-2 text-[11px] text-risk"
            >
              {error}
            </div>
          )}
          {success && (
            <div
              role="status"
              className="border border-emerald-500/40 bg-emerald-500/10 p-2 text-[11px] text-accent-pnl"
            >
              {success}
            </div>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-white/10 flex justify-end gap-2">
          <BrutalButton variant="ghost" onClick={onClose} disabled={submitting}>
            Cancel
          </BrutalButton>
          <BrutalButton
            variant="pnl"
            onClick={handleSubmit}
            disabled={submitting || !!success}
            aria-label="Confirm upgrade"
          >
            {submitting ? "Settling…" : `Confirm — $${next.monthlyUsd}/mo`}
          </BrutalButton>
        </footer>
      </div>
    </div>
  );
}

function Row({
  label,
  value,
  valueClass = "text-text-hi",
}: {
  label: string;
  value: string;
  valueClass?: string;
}) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-text-lo">{label}</span>
      <span className={valueClass}>{value}</span>
    </div>
  );
}
