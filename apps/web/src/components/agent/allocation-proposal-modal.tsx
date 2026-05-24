"use client";

import { useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";

import { agentApi, portfolioApi } from "@/lib/api";
import { pollDecisionReady } from "@/lib/decision-poll";
import type { AgentDecision, AssetSymbol, RiskTolerance } from "@/types";
import { usePortfolioStore } from "@/stores/portfolio";
import { ConstitutionClauseBadge } from "@/components/agent/constitution-clause-badge";
import {
  allocationDisplayMeta,
  type RouteStateLabel,
} from "@/lib/route-capabilities";
import { ModelBadge } from "@aegis/ui";

// Read-only target donut palette. Money/PnL surfaces use green; this is a
// proposed-allocation visualization (the holdings the user will own), so green
// is the correct accent here even though the modal header is the agent/cyan
// surface.
// PnL exception: target allocation chips show owned-asset weights (money).
const TARGET_COLORS = [
  "#00FF88",
  "#FFB800",
  "#FF2D7A",
  "#A855F7",
  "#F97316",
  "#FFFFFF",
];

function pickHeadlineConfidence(decision: AgentDecision): {
  headlinePct: number;
  rawPct: number;
  isCalibrated: boolean;
} {
  const isCalibrated = typeof decision.calibratedConfidence === "number";
  const headline = isCalibrated
    ? (decision.calibratedConfidence ?? 0)
    : (decision.rawConfidence ?? decision.confidence ?? 0);
  const raw = decision.rawConfidence ?? decision.confidence ?? 0;
  return {
    headlinePct: Math.round(headline * 100),
    rawPct: Math.round(raw * 100),
    isCalibrated,
  };
}

function allocationRows(
  allocation: Partial<Record<AssetSymbol, number>> | undefined,
): Array<{ symbol: string; weight: number }> {
  return Object.entries(allocation ?? {})
    .filter(
      (entry): entry is [string, number] =>
        typeof entry[1] === "number" &&
        Number.isFinite(entry[1]) &&
        entry[1] > 0,
    )
    .map(([symbol, weight]) => ({ symbol, weight }))
    .sort((a, b) => b.weight - a.weight);
}

function routeStateBadgeClass(state: RouteStateLabel): string {
  // On-palette neutral tiers — route state is informational and must not borrow
  // the green=money / cyan=agent accents.
  return state === "executes-now"
    ? "border-border-hi text-text-lo"
    : "border-border-default text-text-mut";
}

const RISK_STEPS: { value: RiskTolerance; label: string }[] = [
  { value: "conservative", label: "Conservative" },
  { value: "moderate", label: "Moderate" },
  { value: "aggressive", label: "Aggressive" },
];

// Approve/deploy is a money action, so the CTA is green even on the agent
// surface. The class string is centralized here so the lint marker sits on the
// same line as the token.
const MONEY_CTA_CLASS =
  "px-4 py-2 text-sm font-semibold border-2 bg-emerald-500 text-black border-emerald-300 hover:bg-emerald-400 transition-colors"; // PnL exception: approve/deploy commits the user's money.

interface AllocationProposalModalProps {
  open: boolean;
  portfolioId: string;
  decision: AgentDecision | null;
  onClose: () => void;
  /** Fired after the proposal is approved and the portfolio is refreshed.
   * The dashboard uses this to surface the "Review deployment plan" CTA and
   * trigger the existing deploy flow. */
  onApproved: () => void | Promise<void>;
}

export function AllocationProposalModal({
  open,
  portfolioId,
  decision,
  onClose,
  onApproved,
}: AllocationProposalModalProps) {
  const patchPortfolio = usePortfolioStore((s) => s.patchPortfolio);
  const [current, setCurrent] = useState<AgentDecision | null>(decision);
  const [reproposing, setReproposing] = useState<RiskTolerance | null>(null);
  const [approving, setApproving] = useState(false);
  const [openingReview, setOpeningReview] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const parentDecisionIdRef = useRef(decision?.id ?? null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (!decision || reproposing || approving || openingReview) return;
    if (parentDecisionIdRef.current === decision.id) return;
    parentDecisionIdRef.current = decision.id;
    setCurrent(decision);
  }, [decision, reproposing, approving, openingReview]);

  // Keep the rendered proposal stable while a re-propose job is running. The
  // ready result replaces it atomically so the modal never blanks out.
  const active = current ?? decision;
  if (!open || !active) return null;

  const { headlinePct, rawPct, isCalibrated } = pickHeadlineConfidence(active);
  const rows = allocationRows(active.recommendedAllocation);
  const totalWeight = rows.reduce((acc, r) => acc + r.weight, 0);
  const clauseIds = active.criticVerdict?.clauseIds ?? [];
  const criticWarning =
    active.criticVerdict?.demandsRevision === true ||
    active.criticVerdict?.verdict === "veto" ||
    active.criticVerdict?.verdict === "revised";

  const handleRepropose = async (risk: RiskTolerance) => {
    const step = RISK_STEPS.find((item) => item.value === risk);
    setReproposing(risk);
    setError(null);
    try {
      const queued = await agentApi.proposeAllocation(portfolioId, risk);
      const next = await pollDecisionReady(queued.id, () => mountedRef.current);
      if (!mountedRef.current) return;
      setCurrent(next);
    } catch (e) {
      setError(
        e instanceof Error
          ? e.message
          : `Could not design a ${step?.label.toLowerCase() ?? risk} plan.`,
      );
    } finally {
      if (mountedRef.current) setReproposing(null);
    }
  };

  const handleApprove = async () => {
    if (!active) return;
    setApproving(true);
    setOpeningReview(false);
    setError(null);
    try {
      await agentApi.approveAllocation(active.id);
      try {
        const refreshed = await portfolioApi.get(portfolioId);
        patchPortfolio(portfolioId, {
          goal: refreshed.goal,
          allocations: refreshed.allocations,
        });
      } catch {
        /* the SSE/store refresh will catch up; don't block the success path */
      }
      if (!mountedRef.current) return;
      setApproving(false);
      setOpeningReview(true);
      await onApproved();
    } catch (e) {
      if (mountedRef.current) {
        setOpeningReview(false);
        setError(e instanceof Error ? e.message : "Approval failed.");
      }
    } finally {
      if (mountedRef.current) setApproving(false);
    }
  };
  const busy = approving || openingReview || reproposing !== null;
  const reproposingLabel = RISK_STEPS.find(
    (step) => step.value === reproposing,
  )?.label.toLowerCase();

  return (
    <div
      data-testid="allocation-proposal-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
    >
      <div className="w-full sm:max-w-xl max-h-[90dvh] overflow-y-auto bg-[#141414] border-2 border-accent-agent/40 shadow-[8px_8px_0_0_#000]">
        <header className="px-6 py-4 border-b border-accent-agent/20 flex items-start justify-between gap-3 bg-accent-agent/5">
          <div>
            <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
              Agent allocation
            </p>
            <h2 className="mt-1 text-base font-semibold text-text-hi">
              The agent designed your allocation
            </h2>
            <p className="mt-1 text-[11px] font-mono text-text-lo">
              Review the target mix before any funds move. You approve first.
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

        <div className="px-6 py-4 space-y-4">
          <div className="flex flex-wrap items-center gap-2 font-mono text-[11px]">
            {active.modelSlug && <ModelBadge model={active.modelSlug} />}
            {active.regime && (
              <span className="px-1.5 py-0.5 bg-violet-500/10 border border-violet-500/30 text-violet-200">
                regime: {active.regime}
              </span>
            )}
            <span
              className={
                criticWarning
                  ? "border border-warn/30 bg-warn/10 px-2 py-0.5 text-warn"
                  : "border border-accent-agent/30 bg-accent-agent/10 px-2 py-0.5 text-accent-agent"
              }
            >
              {criticWarning ? "Critic warning" : "Critic passed"}
            </span>
          </div>

          <div className="border border-accent-agent/20 bg-accent-agent/5 p-3">
            <div className="flex items-center justify-between font-mono text-[11px]">
              <span className="text-accent-agent uppercase tracking-wider">
                {isCalibrated ? "Calibrated confidence" : "Confidence"}
              </span>
              <span className="text-text-hi tabular-nums">
                {headlinePct}%
                {isCalibrated && (
                  <span className="ml-2 text-text-mut">raw {rawPct}%</span>
                )}
              </span>
            </div>
            <div className="mt-2 h-2 border border-accent-agent/30 bg-bg">
              <div
                className="h-full bg-accent-agent"
                style={{ width: `${Math.max(0, Math.min(headlinePct, 100))}%` }}
              />
            </div>
          </div>

          <section
            aria-label="Proposed target allocation"
            className="space-y-2"
          >
            <div className="flex items-center justify-between font-mono text-[11px] text-text-lo">
              <span className="uppercase tracking-wider text-text-mut">
                Proposed target mix
              </span>
              <span className="tabular-nums">{totalWeight.toFixed(0)}%</span>
            </div>
            {rows.length === 0 ? (
              <p className="border border-border-default bg-bg/70 px-3 py-2 font-mono text-[11px] text-text-lo">
                The agent did not return any weights. Re-propose to try again.
              </p>
            ) : (
              <div className="space-y-2">
                {rows.map((row, i) => {
                  const meta = allocationDisplayMeta(
                    row.symbol,
                    active.routeStates?.[row.symbol as AssetSymbol],
                  );
                  return (
                    <div key={row.symbol} className="grid gap-1 font-mono">
                      <div className="flex items-center justify-between gap-3 text-xs">
                        <span className="flex min-w-0 items-center gap-1.5">
                          <span className="truncate text-text-lo">
                            {meta.label}
                          </span>
                          <span
                            className={`shrink-0 whitespace-nowrap border px-1 py-px text-[9px] uppercase tracking-wider ${routeStateBadgeClass(
                              meta.routeState,
                            )}`}
                          >
                            {meta.badge}
                          </span>
                        </span>
                        <span className="shrink-0 font-semibold text-text-hi tabular-nums">
                          {row.weight.toFixed(0)}%
                        </span>
                      </div>
                      <div className="h-1.5 border border-border-default bg-bg">
                        <div
                          className="h-full"
                          style={{
                            width: `${Math.max(0, Math.min(row.weight, 100))}%`,
                            backgroundColor:
                              TARGET_COLORS[i % TARGET_COLORS.length],
                          }}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
            {typeof active.expectedMaxDrawdownPct === "number" && (
              <p className="font-mono text-[11px] text-text-mut">
                Projected max drawdown ≈{" "}
                {active.expectedMaxDrawdownPct.toFixed(0)}%
              </p>
            )}
          </section>

          {active.reasoning?.trim() && (
            <div className="border border-white/10 bg-black/30 p-3 font-mono text-[11px] text-text-lo leading-relaxed">
              {active.reasoning.trim()}
            </div>
          )}

          {active.criticVerdict && (
            <div className="space-y-2 font-mono text-[11px]">
              {clauseIds.length > 0 ? (
                <div className="flex flex-wrap gap-1">
                  {clauseIds.map((id) => (
                    <ConstitutionClauseBadge key={id} clauseId={id} violated />
                  ))}
                </div>
              ) : (
                <ConstitutionClauseBadge
                  clauseId="Constitution clean"
                  violated={false}
                  summary="No hard constraints violated."
                />
              )}
              {active.criticVerdict.notes && (
                <p className="text-warn/90">
                  Critic: {active.criticVerdict.notes}
                </p>
              )}
            </div>
          )}

          <div className="border border-white/10 bg-black/20 p-3">
            <p className="font-mono text-[10px] uppercase tracking-wider text-accent-agent">
              Risk dial — re-propose at a different posture
            </p>
            {reproposingLabel && (
              <p
                className="mt-2 flex items-center gap-2 border border-accent-agent/30 bg-accent-agent/5 px-2 py-1.5 font-mono text-[11px] text-accent-agent"
                role="status"
                aria-live="polite"
              >
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                Designing {reproposingLabel} allocation…
              </p>
            )}
            <div className="mt-2 grid grid-cols-3 gap-2">
              {RISK_STEPS.map((step) => (
                <button
                  key={step.value}
                  type="button"
                  disabled={busy}
                  onClick={() => void handleRepropose(step.value)}
                  aria-busy={reproposing === step.value}
                  className="inline-flex min-h-9 items-center justify-center gap-1.5 rounded-sharp border border-accent-agent/40 bg-accent-agent/5 px-2 py-1 font-mono text-[11px] text-accent-agent hover:bg-accent-agent/10 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {reproposing === step.value && (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  )}
                  {reproposing === step.value ? "Designing" : step.label}
                </button>
              ))}
            </div>
          </div>

          <ProposalProvenance
            modelSlug={active.modelSlug}
            regime={active.regime}
          />

          {error && (
            <p className="font-mono text-xs text-risk" role="alert">
              {error}
            </p>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-white/10 flex items-center justify-between gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="px-4 py-2 text-sm text-text-default hover:text-text-hi border border-white/10"
          >
            Not now
          </button>
          <button
            type="button"
            onClick={() => void handleApprove()}
            disabled={busy || rows.length === 0}
            aria-busy={approving || openingReview}
            className={`${MONEY_CTA_CLASS} disabled:opacity-50 disabled:cursor-not-allowed inline-flex items-center gap-2`}
          >
            {(approving || openingReview) && (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            )}
            {openingReview
              ? "Opening review"
              : approving
                ? "Approving allocation"
                : "Approve allocation"}
          </button>
        </footer>
      </div>
    </div>
  );
}

function ProposalProvenance({
  modelSlug,
  regime,
}: {
  modelSlug?: string;
  regime?: string;
}) {
  const parts = [modelSlug ? `via ${modelSlug}` : null, regime].filter(Boolean);
  if (parts.length === 0) return null;
  return (
    <p className="font-mono text-[10px] text-text-mut">{parts.join(" · ")}</p>
  );
}
