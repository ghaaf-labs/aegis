"use client";

import { BrutalButton, BrutalCard, BrutalPill } from "@aegis/ui";
import { cn } from "@/lib/utils";
import type { PricingTier, Tier } from "@/types";

/** Static fallback rendered when the API has not (yet) returned tiers —
 * numbers mirror §2.1 of the roadmap exactly so the page is never blank. */
export const DEFAULT_PRICING_TIERS: PricingTier[] = [
  {
    code: "free",
    tier: "free",
    name: "Free",
    monthlyUsd: 0,
    aumCapUsd: 5_000,
    portfolioCap: 1,
    portfoliosCap: 1,
    decisionsPerMonth: 5,
    decisionsCapMonthly: 5,
    models: "Haiku regime + Haiku strategist",
    perRebalanceBps: 25,
    aumAnnualBps: 0,
    features: [
      "1 portfolio",
      "5 agent decisions / month",
      "USDC fee preview on every move",
      "Paymaster gas eaten by Aegis",
    ],
  },
  {
    code: "pro",
    tier: "pro",
    name: "Pro",
    monthlyUsd: 19,
    aumCapUsd: null,
    portfolioCap: 1,
    portfoliosCap: 1,
    decisionsPerMonth: 240,
    decisionsCapMonthly: 240,
    models: "Haiku + Opus 4.7 strategist + GPT-5.5 critic",
    perRebalanceBps: 15,
    aumAnnualBps: 25,
    features: [
      "240 decisions / month",
      "Critic-audited proposals",
      "Auto-execute + peg defense",
      "Counterfactual + calibrated confidence",
    ],
    recommended: true,
  },
  {
    code: "business",
    tier: "business",
    name: "Business",
    monthlyUsd: 199,
    aumCapUsd: null,
    portfolioCap: 1,
    portfoliosCap: 1,
    decisionsCapMonthly: null,
    decisionsPerMonth: null,
    models: "Pro models + Constitution + counterfactual",
    perRebalanceBps: 10,
    aumAnnualBps: 15,
    features: [
      "Unlimited decisions",
      "Constitution-bound critic",
      "1099-DA tax export + accountant share link",
      "Priority support + SLA",
    ],
  },
];

function formatLimit(value: number | null, unit: string): string {
  if (value === null) return "Unlimited";
  return `${value.toLocaleString()} ${unit}`;
}

function formatAumCap(value: number | null): string {
  if (value === null) return "Unlimited";
  if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `$${(value / 1_000).toFixed(0)}K`;
  return `$${value}`;
}

interface PricingTableProps {
  tiers?: PricingTier[];
  /** When present, the matching column is decorated with a "Your plan" pill
   * and its CTA is disabled. */
  currentTier?: Tier | null;
  /** Click handler for the Upgrade CTA. Receives the target tier slug.
   * When omitted (e.g. on the public /pricing page), the CTA links to
   * /login via an <a>. */
  onSelect?: (tier: Tier) => void;
  /** Optional disabled state, e.g. while a Nanopayments call is in-flight. */
  busyTier?: Tier | null;
  /** Public-page CTA overrides used when auth is unavailable or contextual. */
  publicActionHref?: string;
  publicActionLabel?: string | ((tier: PricingTier) => string);
  publicActionTone?: "pnl" | "agent";
  publicActionHint?: string | null;
  actionsDisabled?: boolean;
  disabledActionLabel?: string;
}

export function PricingTable({
  tiers = DEFAULT_PRICING_TIERS,
  currentTier = null,
  onSelect,
  busyTier = null,
  publicActionHref = "/login",
  publicActionLabel,
  publicActionTone = "pnl",
  publicActionHint = null,
  actionsDisabled = false,
  disabledActionLabel = "Unavailable",
}: PricingTableProps) {
  return (
    <div className="grid gap-4 md:grid-cols-3" data-testid="pricing-table">
      {tiers.map((t) => {
        const isCurrent = currentTier === t.tier;
        const isPro = t.tier === "pro" || t.recommended;
        const isBusy = busyTier === t.tier;
        return (
          <BrutalCard
            key={t.tier}
            variant={isPro ? "raised" : "default"}
            className={cn(
              "flex flex-col p-5",
              isPro && "shadow-brutal -translate-y-1",
            )}
            data-tier={t.tier}
          >
            <header className="flex items-center justify-between mb-3">
              <BrutalPill tone="agent">
                {(t.name ?? t.code).toUpperCase()}
              </BrutalPill>
              {isCurrent && <BrutalPill tone="pnl">YOUR PLAN</BrutalPill>}
              {!isCurrent && isPro && (
                <BrutalPill tone="pnl">RECOMMENDED</BrutalPill>
              )}
            </header>

            <div className="mb-4 font-mono">
              <span className="text-3xl font-bold text-text-hi tabular-nums">
                ${t.monthlyUsd}
              </span>
              <span className="text-text-lo text-sm">/mo</span>
            </div>

            <dl className="space-y-1.5 text-xs font-mono text-text-default mb-5">
              <Row label="AUM cap" value={formatAumCap(t.aumCapUsd)} />
              <Row
                label="Decisions / mo"
                value={formatLimit(
                  t.decisionsPerMonth ?? t.decisionsCapMonthly,
                  "",
                )}
              />
              <Row label="Models" value={t.models ?? ""} />
              <Row label="Per-rebalance" value={`${t.perRebalanceBps} bps`} />
              <Row
                label="AUM fee (annual)"
                value={t.aumAnnualBps === 0 ? "—" : `${t.aumAnnualBps} bps`}
              />
            </dl>

            <ul className="space-y-1.5 text-xs text-text-default mb-5 flex-1">
              {(t.features ?? []).map((f: string) => (
                <li key={f} className="flex items-start gap-2">
                  <BrutalPill tone="neutral" className="shrink-0">
                    ✓
                  </BrutalPill>
                  <span className="leading-relaxed">{f}</span>
                </li>
              ))}
            </ul>

            {onSelect ? (
              <BrutalButton
                variant="pnl"
                disabled={isCurrent || isBusy || actionsDisabled}
                onClick={() => onSelect((t.tier ?? t.code) as Tier)}
                aria-label={
                  isCurrent
                    ? `Currently on ${t.name ?? t.code}`
                    : actionsDisabled
                      ? `${t.name ?? t.code} is unavailable`
                      : `Upgrade to ${t.name ?? t.code}`
                }
              >
                {isCurrent
                  ? "Current plan"
                  : actionsDisabled
                    ? disabledActionLabel
                    : isBusy
                      ? "Settling…"
                      : t.tier === "free"
                        ? "Downgrade"
                        : `Upgrade to ${t.name ?? t.code}`}
              </BrutalButton>
            ) : (
              <a
                href={publicActionHref}
                className={cn(
                  "inline-flex w-full items-center justify-center gap-2 px-3 py-2 text-sm font-semibold",
                  "border-brutal border-black rounded-sharp text-black",
                  publicActionTone === "agent"
                    ? "bg-accent-agent"
                    : "bg-accent-pnl",
                  "transition-[box-shadow,transform] duration-100 hover:shadow-brutal-sm active:translate-y-px",
                )}
              >
                {publicActionLabel
                  ? typeof publicActionLabel === "function"
                    ? publicActionLabel(t)
                    : publicActionLabel
                  : t.tier === "free"
                    ? "Get started — free"
                    : `Choose ${t.name ?? t.code}`}
              </a>
            )}
            {!onSelect && publicActionHint && (
              <p className="mt-2 text-[11px] font-mono leading-relaxed text-text-mut">
                {publicActionHint}
              </p>
            )}
          </BrutalCard>
        );
      })}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3">
      <dt className="text-text-lo">{label}</dt>
      <dd className="text-text-hi text-right">{value}</dd>
    </div>
  );
}
