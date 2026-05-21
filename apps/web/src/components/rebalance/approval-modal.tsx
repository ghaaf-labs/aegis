"use client";

import { useState } from "react";

import {
  rebalanceApi,
  type RebalanceApprovalSafety,
  type RebalancePlanResponse,
} from "@/lib/api";
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
  approvalSafety?: RebalanceApprovalSafety | null;
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

function routedAmountUsdc(plan: RebalancePlanResponse): number {
  return plan.legs
    .filter((leg) => leg.kind !== "cross_chain_mint")
    .reduce((acc, leg) => acc + leg.amountUsdc, 0);
}

function destinationAmounts(plan: RebalancePlanResponse): Array<{
  symbol: string;
  amountUsdc: number;
}> {
  const totals = new Map<string, number>();
  for (const leg of plan.legs) {
    if (leg.kind === "cross_chain_mint") continue;
    if (!leg.destSymbol || leg.destSymbol === "USDC") continue;
    totals.set(
      leg.destSymbol,
      (totals.get(leg.destSymbol) ?? 0) + leg.amountUsdc,
    );
  }
  return Array.from(totals.entries())
    .map(([symbol, amountUsdc]) => ({ symbol, amountUsdc }))
    .sort((a, b) => b.amountUsdc - a.amountUsdc);
}

function sourceAmounts(plan: RebalancePlanResponse): Array<{
  symbol: string;
  amountUsdc: number;
}> {
  const totals = new Map<string, number>();
  for (const leg of plan.legs) {
    if (!leg.srcSymbol || leg.srcSymbol === "USDC") continue;
    if (leg.destSymbol !== "USDC") continue;
    totals.set(
      leg.srcSymbol,
      (totals.get(leg.srcSymbol) ?? 0) + leg.amountUsdc,
    );
  }
  return Array.from(totals.entries())
    .map(([symbol, amountUsdc]) => ({ symbol, amountUsdc }))
    .sort((a, b) => b.amountUsdc - a.amountUsdc);
}

function bridgedAmountUsdc(plan: RebalancePlanResponse): number {
  return plan.legs
    .filter((leg) => leg.kind === "cross_chain_burn")
    .reduce((acc, leg) => acc + leg.amountUsdc, 0);
}

export function ApprovalModal({
  open,
  plan,
  portfolioId,
  estimatedFeeUsdc,
  feeFetchedAt,
  feeSource = "plan",
  portfolioName,
  decision,
  approvalSafety,
  onApproved,
  onClose,
}: ApprovalModalProps) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [counterfactualOpen, setCounterfactualOpen] = useState(false);
  const [routeOpen, setRouteOpen] = useState(false);

  if (!open || !plan) return null;

  const routedUsdc = routedAmountUsdc(plan);
  const isMockExecution = plan.executionMode === "mock";
  const destinations = destinationAmounts(plan);
  const sources = sourceAmounts(plan);
  const bridgedUsdc = bridgedAmountUsdc(plan);
  const hasPositionSales = sources.length > 0;
  const approvalBlocked = approvalSafety?.approvable === false;
  const changeHeadline =
    plan.totalLegs === 0
      ? "No portfolio changes needed"
      : hasPositionSales
        ? `Rebalance ${routedUsdc.toFixed(2)} USD across positions`
        : destinations.length > 0
          ? `Deploy $${routedUsdc.toFixed(2)} of wallet USDC`
          : `Route $${routedUsdc.toFixed(2)} USDC`;

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
    <div
      data-testid="approval-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
    >
      <div className="w-full sm:max-w-xl max-h-[90dvh] overflow-y-auto bg-[#141414] border-2 border-white/15 shadow-[8px_8px_0_0_#000]">
        <header className="px-6 py-4 border-b border-white/10 flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-text-hi">
              Approve rebalance
            </h2>
            {portfolioName && (
              <p className="text-[11px] font-mono text-text-lo mt-1">
                {portfolioName}
              </p>
            )}
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

        <div className="px-6 py-4">
          <div className="mb-4 border-2 border-accent-agent/30 bg-cyan-500/5 p-4">
            <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
              What will change
            </p>
            <h3 className="mt-1 text-lg font-semibold text-text-hi">
              {changeHeadline}
            </h3>
            <div className="mt-3 grid gap-2 text-xs font-mono text-text-lo">
              {sources.map((item) => (
                <div
                  key={`source-${item.symbol}`}
                  className="flex items-center justify-between border border-risk/20 bg-risk/5 px-3 py-2 text-risk"
                >
                  <span>Sell / redeem {item.symbol}</span>
                  <span>${item.amountUsdc.toFixed(2)}</span>
                </div>
              ))}
              {destinations.length > 0 ? (
                destinations.map((item) => (
                  <div
                    key={`dest-${item.symbol}`}
                    className="flex items-center justify-between border border-white/10 bg-black/30 px-3 py-2"
                  >
                    <span>Buy / park {item.symbol}</span>
                    <span className="text-accent-pnl">
                      ${item.amountUsdc.toFixed(2)}
                    </span>
                  </div>
                ))
              ) : (
                <div className="border border-white/10 bg-black/30 px-3 py-2">
                  No buy or park leg is needed. The plan only moves existing
                  exposure.
                </div>
              )}
              {bridgedUsdc > 0 && (
                <div className="flex items-center justify-between border border-cyan-500/20 bg-cyan-500/5 px-3 py-2 text-accent-agent">
                  <span>
                    {isMockExecution ? "Simulate bridge" : "Bridge"} Arc → Base
                  </span>
                  <span>${bridgedUsdc.toFixed(2)}</span>
                </div>
              )}
            </div>
            <p className="mt-3 text-[11px] leading-relaxed text-text-lo">
              {approvalBlocked
                ? "These amounts are historical and cannot be executed from this screen. Build a fresh review to see the current wallet and position route."
                : isMockExecution
                  ? "This updates the local demo portfolio and mock Gateway balance so you can see the state change immediately."
                  : hasPositionSales
                    ? "This approval sells overweight positions, routes USDC, and buys or parks underweight targets. It is not idle-wallet deployment."
                    : "This approval uses wallet USDC for real execution after you confirm."}
            </p>
          </div>

          {approvalBlocked && (
            <div className="mb-4 border-brutal border-warn/50 bg-warn/10 p-3 text-xs font-mono text-warn">
              <p className="text-[10px] uppercase tracking-wider">
                Approval blocked · {approvalSafety.code}
              </p>
              <p className="mt-1 leading-relaxed">{approvalSafety.message}</p>
              <a
                href={portfolioId ? `/dashboard/${portfolioId}` : "/dashboard"}
                className="mt-2 inline-flex border border-warn/40 px-2 py-1 text-[11px] text-warn hover:bg-warn/10"
              >
                Open Dashboard for fresh review
              </a>
            </div>
          )}

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
                    <span className="text-accent-agent uppercase tracking-wider">
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
                      className="ml-auto text-text-lo"
                      title={
                        isCalibrated
                          ? `Histogram-bin calibrated · raw ${rawPct}%`
                          : "Raw strategist confidence — no calibration available yet"
                      }
                    >
                      {isCalibrated ? "calibrated " : "confidence "}
                      <span className="text-accent-agent">{headlinePct}%</span>
                      {isCalibrated && (
                        <span className="text-text-mut"> (raw {rawPct}%)</span>
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
                    <p className="text-text-default leading-relaxed">
                      {decision.reasoning}
                    </p>
                  )}

                  {/* F-CON-4: constitution clause IDs (veto reasons). */}
                  {clauseIds.length > 0 && (
                    <div className="border-t border-white/5 pt-2 space-y-1">
                      <p className="text-[10px] uppercase tracking-wider text-risk">
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
                    <p className="text-[10px] text-warn/90 border-t border-white/5 pt-2">
                      <span className="uppercase tracking-wider text-warn mr-1.5">
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
                        className="text-[10px] uppercase tracking-wider text-accent-agent hover:text-accent-agent/70"
                      >
                        {counterfactualOpen ? "▾" : "▸"} Why this might be wrong
                      </button>
                      {counterfactualOpen && (
                        <p className="mt-1.5 text-[11px] text-accent-agent/60 bg-cyan-500/5 border border-cyan-500/20 px-2 py-1.5">
                          {decision.counterfactual}
                        </p>
                      )}
                    </div>
                  )}
                  {isCalibrated && (
                    <p
                      className="text-[10px] text-text-mut"
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
            <div className="mb-3 inline-flex items-center gap-2 rounded border border-cyan-500/40 bg-cyan-500/10 px-3 py-1 text-[11px] font-mono text-accent-agent">
              {isMockExecution
                ? "Local demo execution • simulates CCTP V2 + Hooks"
                : "Real on-chain execution • CCTP V2 Fast Transfer + Hooks"}
            </div>
          )}

          <p className="text-sm text-text-default mb-3">
            {approvalBlocked ? (
              <>
                Aegis is showing these <strong>{plan.totalLegs}</strong> stale
                leg{plan.totalLegs === 1 ? "" : "s"} for audit only. Build a
                fresh review before any execution.
              </>
            ) : (
              <>
                The agent has planned <strong>{plan.totalLegs}</strong> leg
                {plan.totalLegs === 1 ? "" : "s"} to bring your portfolio toward
                its target.{" "}
                {isMockExecution
                  ? "This local demo updates mock positions and Gateway balances; no real chain transaction is sent."
                  : "One approval settles the plan on Arc + Base; SSE streams each leg as it confirms."}
              </>
            )}
          </p>

          <div className="mb-4 border border-white/10 bg-black/20">
            <button
              type="button"
              onClick={() => setRouteOpen((v) => !v)}
              className="flex w-full items-center justify-between px-3 py-2 text-left text-xs font-mono text-text-hi hover:bg-white/5"
            >
              <span>Technical route</span>
              <span className="text-text-mut">
                {plan.totalLegs} leg{plan.totalLegs === 1 ? "" : "s"}{" "}
                {routeOpen ? "shown" : "hidden"}
              </span>
            </button>
            {routeOpen && (
              <ol className="space-y-2 border-t border-white/10 p-3">
                {plan.legs.map((leg) => (
                  <li
                    key={leg.legIndex}
                    data-testid="leg-card"
                    className="flex justify-between text-xs font-mono border border-white/5 p-2"
                  >
                    <span className="flex items-center gap-1.5">
                      <span className="text-text-mut">
                        {String(leg.legIndex + 1).padStart(2, "0")}
                      </span>
                      <span className="text-text-hi">
                        {KIND_LABEL[leg.kind] ?? leg.kind}
                      </span>
                      {leg.srcChain && (
                        <ChainBadge
                          chain={
                            leg.srcChain.toUpperCase() as
                              | "ARC"
                              | "BASE"
                              | "AVAX"
                          }
                        />
                      )}
                      {leg.destChain && leg.destChain !== leg.srcChain && (
                        <ChainBadge
                          chain={
                            leg.destChain.toUpperCase() as
                              | "ARC"
                              | "BASE"
                              | "AVAX"
                          }
                        />
                      )}
                    </span>
                    <span className="text-text-lo">
                      {leg.srcSymbol} → {leg.destSymbol}
                      <span className="text-accent-pnl ml-2">
                        ${leg.amountUsdc.toFixed(2)}
                      </span>
                    </span>
                  </li>
                ))}
              </ol>
            )}
          </div>

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
            <div className="flex justify-between text-text-lo">
              <span title="Indicative gas estimate — not a binding quote. Actual settlement fee is published in the trace once the leg confirms.">
                Paymaster (USDC gas)*
              </span>
              <span className="text-accent-pnl">
                ≈ ${estimatedFeeUsdc.toFixed(4)} USDC
              </span>
            </div>
            <div className="flex justify-between text-text-lo mt-1">
              <span>Total amount routed</span>
              <span className="text-text-hi">${routedUsdc.toFixed(2)}</span>
            </div>
            <div className="text-[10px] text-text-mut mt-2">
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
                <span className="text-warn">
                  Protocol fee (25 bps via Nanopayments x402)
                </span>
                <span className="font-mono text-warn">
                  ≈ ${(routedUsdc * 0.0025).toFixed(4)} USDC
                </span>
              </div>
            )}

            {/* Total estimated cost to user */}
            {plan && plan.legs && plan.legs.length > 0 && (
              <div className="mt-2 pt-2 border-t border-white/10 flex justify-between text-sm font-semibold">
                <span className="text-text-hi">Total estimated cost</span>
                <span className="font-mono text-text-hi">
                  ≈ ${(estimatedFeeUsdc + routedUsdc * 0.0025).toFixed(4)} USDC
                </span>
              </div>
            )}
          </div>

          {error && (
            <p className="text-xs text-risk font-mono mb-3" role="alert">
              {error}
            </p>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-white/10 flex items-center justify-between gap-2">
          <a
            href="/policy"
            target="_blank"
            rel="noopener"
            className="text-xs text-text-lo hover:text-text-hi underline-offset-4 hover:underline"
          >
            Outcome &amp; refund policy
          </a>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm text-text-default hover:text-text-hi border border-white/10"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleApprove}
              disabled={submitting || approvalBlocked}
              className={cn(
                "px-4 py-2 text-sm font-semibold border-2",
                approvalBlocked
                  ? "bg-warn/20 text-warn border-warn/40"
                  : "bg-emerald-500 text-black border-emerald-300 hover:bg-emerald-400",
                "transition-colors",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              )}
            >
              {approvalBlocked
                ? "Approval blocked"
                : submitting
                  ? "Submitting…"
                  : isMockExecution
                    ? "Run local execution"
                    : "Approve & execute"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
