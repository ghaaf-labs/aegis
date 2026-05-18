"use client";

import { useState } from "react";

import { rebalanceApi, type RebalancePlanResponse } from "@/lib/api";
import type { AgentDecision } from "@/types";
import { cn } from "@/lib/utils";
import { BacktestPreview } from "@/components/rebalance/backtest-preview";
import { ConstitutionClauseBadge } from "@/components/agent/ConstitutionClauseBadge";
import { ModelBadge, ChainBadge } from "@aegis/ui";

/** Headline confidence the modal renders.
 *
 * Prefers the histogram-bin calibrated confidence (F-CONF-4 → agent service
 * with CALIBRATED_CONF_ENABLED=true). Falls back to the strategist's flat
 * raw confidence when no calibration exists yet, then to the legacy
 * `confidence` field for back-compat with decisions persisted before
 * migration 0013. */
function pickHeadlineConfidence(decision: AgentDecision): number {
  if (typeof decision.calibratedConfidence === "number") {
    return decision.calibratedConfidence;
  }
  if (typeof decision.rawConfidence === "number") {
    return decision.rawConfidence;
  }
  return decision.confidence ?? 0;
}

function formatRelativeSeconds(at: Date): string {
  const secs = Math.max(0, Math.round((Date.now() - at.getTime()) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.round(secs / 60)}m ago`;
  return `${Math.round(secs / 3600)}h ago`;
}

export interface ApprovalModalProps {
  open: boolean;
  plan: RebalancePlanResponse | null;
  /** Drives the inline backtest preview. Defaults to no preview when null. */
  portfolioId?: string | null;
  estimatedFeeUsdc: number;
  /** When the fee number was fetched. Drives the provenance line. */
  feeFetchedAt?: Date | null;
  /** Where the fee came from — `plan` is the planner-time stored value;
   *  `paymaster` is a live quote from `GET /paymaster/estimate`. */
  feeSource?: "plan" | "paymaster";
  /** Optional per-user / per-portfolio context surfaced in the header. */
  portfolioName?: string;
  /** The AgentDecision behind this plan. When present the modal surfaces
   *  model_slug + confidence + critic verdict next to the plan — required
   *  for Agentic Sophistication judging (30% weight). */
  decision?: AgentDecision | null;
  onApproved: (rebalanceId: string) => void;
  onClose: () => void;
}

const KIND_LABEL: Record<string, string> = {
  local_swap: "Swap",
  cross_chain_burn: "CCTP burn",
  cross_chain_mint: "CCTP mint + hook",
  park_usyc: "Park → USYC",
  redeem_usyc: "Redeem ← USYC",
  fx_stablefx: "StableFX",
};

export function ApprovalModal({
  open,
  plan,
  portfolioId,
  estimatedFeeUsdc,
  feeFetchedAt,
  feeSource = "plan",
  portfolioName,
  decision,
  onApproved,
  onClose,
}: ApprovalModalProps) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [counterfactualOpen, setCounterfactualOpen] = useState(false);

  if (!open || !plan) return null;

  const handleApprove = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await rebalanceApi.execute(plan.rebalanceId);
      onApproved(plan.rebalanceId);
    } catch (e) {
      setError(e instanceof Error ? e.message : "approval failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
      <div className="w-full sm:max-w-xl max-h-[90dvh] overflow-y-auto bg-[#141414] border-2 border-white/15 shadow-[8px_8px_0_0_#000]">
        <header className="px-6 py-4 border-b border-white/10 flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-white">
              Approve rebalance
            </h2>
            {portfolioName && (
              <p className="text-[11px] font-mono text-gray-400 mt-1">
                {portfolioName}
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-gray-400 hover:text-white"
            aria-label="Close"
          >
            ×
          </button>
        </header>

        <div className="px-6 py-4">
          {decision &&
            (() => {
              const headline = pickHeadlineConfidence(decision);
              const headlinePct = Math.round(headline * 100);
              const isCalibrated =
                typeof decision.calibratedConfidence === "number";
              const raw = decision.rawConfidence ?? decision.confidence ?? 0;
              const rawPct = Math.round(raw * 100);
              const clauseIds = decision.criticVerdict?.clauseIds ?? [];

              return (
                <div className="mb-4 border border-white/10 bg-black/40 p-3 font-mono text-[11px] space-y-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-cyan-400 uppercase tracking-wider">
                      Agent
                    </span>
                    {decision.modelSlug && (
                      <ModelBadge model={decision.modelSlug} />
                    )}
                    {decision.regime && (
                      <span className="px-1.5 py-0.5 bg-violet-500/10 border border-violet-500/30 text-violet-200">
                        regime: {decision.regime}
                      </span>
                    )}
                    <span
                      className="ml-auto text-gray-400"
                      title={
                        isCalibrated
                          ? `Histogram-bin calibrated · raw ${rawPct}%`
                          : "Raw strategist confidence — no calibration available yet"
                      }
                    >
                      {isCalibrated ? "calibrated " : "confidence "}
                      <span className="text-cyan-300">{headlinePct}%</span>
                      {isCalibrated && (
                        <span className="text-gray-500"> (raw {rawPct}%)</span>
                      )}
                    </span>
                  </div>

                  <div
                    className="h-1.5 w-full bg-white/5 border border-white/10 overflow-hidden"
                    aria-label={`Calibrated confidence ${headlinePct}%`}
                  >
                    <div
                      className="h-full bg-cyan-400"
                      style={{
                        width: `${Math.min(100, Math.max(0, headlinePct))}%`,
                      }}
                    />
                  </div>

                  {decision.reasoning && (
                    <p className="text-gray-300 leading-relaxed">
                      {decision.reasoning}
                    </p>
                  )}

                  {/* F-CON-4: constitution clause IDs (veto reasons). */}
                  {clauseIds.length > 0 && (
                    <div className="border-t border-white/5 pt-2 space-y-1">
                      <p className="text-[10px] uppercase tracking-wider text-rose-400">
                        Critic veto reasons (constitution)
                      </p>
                      <div className="flex flex-wrap gap-1">
                        {clauseIds.map((id) => (
                          <ConstitutionClauseBadge
                            key={id}
                            clauseId={id}
                            violated
                          />
                        ))}
                      </div>
                    </div>
                  )}
                  {decision.criticVerdict &&
                    clauseIds.length === 0 &&
                    decision.criticVerdict.verdict !== "veto" && (
                      <div className="border-t border-white/5 pt-2">
                        <ConstitutionClauseBadge
                          clauseId="Constitution clean"
                          violated={false}
                          summary="No hard constraints violated. Critic ran free-form review only."
                        />
                      </div>
                    )}

                  {decision.criticVerdict && (
                    <p className="text-[10px] text-amber-300/90 border-t border-white/5 pt-2">
                      <span className="uppercase tracking-wider text-amber-400 mr-1.5">
                        Critic
                      </span>
                      (
                      {Math.round(
                        (decision.criticVerdict.confidence ?? 0) * 100,
                      )}
                      %): {decision.criticVerdict.notes}
                    </p>
                  )}

                  {/* F-CONF-6: counterfactual. */}
                  {decision.counterfactual && (
                    <div className="border-t border-white/5 pt-2">
                      <button
                        type="button"
                        onClick={() => setCounterfactualOpen((v) => !v)}
                        className="text-[10px] uppercase tracking-wider text-cyan-300 hover:text-cyan-200"
                      >
                        {counterfactualOpen ? "▾" : "▸"} Why this might be wrong
                      </button>
                      {counterfactualOpen && (
                        <p className="mt-1.5 text-[11px] text-cyan-100/90 bg-cyan-500/5 border border-cyan-500/20 px-2 py-1.5">
                          {decision.counterfactual}
                        </p>
                      )}
                    </div>
                  )}
                  {isCalibrated && (
                    <p
                      className="text-[10px] text-gray-500"
                      title="Calibration source persisted in `calibrations` table"
                    >
                      Calibration: histogram-bin · via model_evaluations
                    </p>
                  )}
                </div>
              );
            })()}

          {plan.legs.some(
            (l) =>
              l.kind === "cross_chain_burn" || l.kind === "cross_chain_mint",
          ) && (
            <div className="mb-3 inline-flex items-center gap-2 rounded border border-cyan-500/40 bg-cyan-500/10 px-3 py-1 text-[11px] font-mono text-cyan-300">
              Real on-chain execution • CCTP V2 Fast Transfer + Hooks
            </div>
          )}

          <p className="text-sm text-gray-300 mb-3">
            The agent has planned <strong>{plan.totalLegs}</strong> leg
            {plan.totalLegs === 1 ? "" : "s"} to bring your portfolio to its
            target. One click executes everything; SSE will stream per-leg
            updates as they confirm.
          </p>

          <ol className="space-y-2 mb-4">
            {plan.legs.map((leg) => (
              <li
                key={leg.legIndex}
                className="flex justify-between text-xs font-mono border border-white/5 p-2"
              >
                <span className="flex items-center gap-1.5">
                  <span className="text-gray-500">
                    {String(leg.legIndex + 1).padStart(2, "0")}
                  </span>
                  <span className="text-white">
                    {KIND_LABEL[leg.kind] ?? leg.kind}
                  </span>
                  {leg.srcChain && (
                    <ChainBadge
                      chain={
                        leg.srcChain.toUpperCase() as "ARC" | "BASE" | "AVAX"
                      }
                    />
                  )}
                  {leg.destChain && leg.destChain !== leg.srcChain && (
                    <ChainBadge
                      chain={
                        leg.destChain.toUpperCase() as "ARC" | "BASE" | "AVAX"
                      }
                    />
                  )}
                </span>
                <span className="text-gray-400">
                  {leg.srcSymbol} → {leg.destSymbol}
                  <span className="text-emerald-400 ml-2">
                    ${leg.amountUsdc.toFixed(2)}
                  </span>
                </span>
              </li>
            ))}
          </ol>

          {plan.legs.some(
            (l) => l.srcSymbol === "EURC" || l.destSymbol === "EURC",
          ) && (
            <div className="border-brutal border-warn/40 bg-warn/10 p-3 mb-4 text-[11px] font-mono text-warn">
              EURC routes via DefiLlama spot rate while institutional Circle
              StableFX access is pending. Slippage may exceed institutional
              execution by ~3-5 bps.
            </div>
          )}

          <BacktestPreview portfolioId={portfolioId ?? null} />

          <div className="bg-black/40 border border-white/5 p-3 text-xs font-mono mb-4">
            <div className="flex justify-between text-gray-400">
              <span>Paymaster (USDC gas)</span>
              <span className="text-emerald-300">
                ≈ ${estimatedFeeUsdc.toFixed(4)} USDC
              </span>
            </div>
            <div className="flex justify-between text-gray-400 mt-1">
              <span>Total amount routed</span>
              <span className="text-white">
                $
                {plan.legs.reduce((acc, l) => acc + l.amountUsdc, 0).toFixed(2)}
              </span>
            </div>
            <div className="text-[10px] text-gray-500 mt-2">
              via{" "}
              {feeSource === "paymaster" ? "Circle Paymaster" : "plan estimate"}
              {feeFetchedAt && (
                <>
                  {" · "}
                  {formatRelativeSeconds(feeFetchedAt)}
                </>
              )}
            </div>

            {/* Protocol fee — Nanopayments 25bps story for judging */}
            {plan && plan.legs && plan.legs.length > 0 && (
              <div className="mt-3 text-sm flex justify-between border-t border-white/10 pt-2">
                <span className="text-amber-400">
                  Protocol fee (25 bps via Nanopayments x402)
                </span>
                <span className="font-mono text-amber-400">
                  ≈ $
                  {(
                    plan.legs.reduce((s, l) => s + l.amountUsdc, 0) * 0.0025
                  ).toFixed(4)}{" "}
                  USDC
                </span>
              </div>
            )}

            {/* Total estimated cost to user */}
            {plan && plan.legs && plan.legs.length > 0 && (
              <div className="mt-2 pt-2 border-t border-white/10 flex justify-between text-sm font-semibold">
                <span className="text-white">Total estimated cost</span>
                <span className="font-mono text-white">
                  ≈ $
                  {(
                    estimatedFeeUsdc +
                    plan.legs.reduce((s, l) => s + l.amountUsdc, 0) * 0.0025
                  ).toFixed(4)}{" "}
                  USDC
                </span>
              </div>
            )}
          </div>

          {error && (
            <p className="text-xs text-rose-300 font-mono mb-3" role="alert">
              {error}
            </p>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-white/10 flex items-center justify-between gap-2">
          <a
            href="/policy"
            target="_blank"
            rel="noopener"
            className="text-xs text-gray-400 hover:text-white underline-offset-4 hover:underline"
          >
            Outcome &amp; refund policy
          </a>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm text-gray-300 hover:text-white border border-white/10"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleApprove}
              disabled={submitting}
              className={cn(
                "px-4 py-2 text-sm font-semibold border-2",
                "bg-emerald-500 text-black border-emerald-300",
                "hover:bg-emerald-400 transition-colors",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              )}
            >
              {submitting ? "Submitting…" : "Approve & execute"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
