"use client";

import { useState } from "react";

import {
  rebalanceApi,
  type RebalanceApprovalSafety,
  type RebalancePlanResponse,
} from "@/lib/api";
import type { AgentDecision } from "@/types";
import { cn } from "@/lib/utils";
import { walletRouteBadgeLabel } from "@/lib/wallet-routes";
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

function chainDestinationTotals(plan: RebalancePlanResponse) {
  const totals = new Map<string, number>();
  for (const leg of plan.legs) {
    if (leg.kind === "cross_chain_mint") continue;
    if (!leg.destSymbol || leg.destSymbol === "USDC") continue;
    const chain = leg.destChain ?? leg.srcChain ?? "arc";
    totals.set(chain, (totals.get(chain) ?? 0) + leg.amountUsdc);
  }
  return {
    arc: totals.get("arc") ?? 0,
    base: totals.get("base") ?? 0,
  };
}

function chainSourceTotals(plan: RebalancePlanResponse) {
  const totals = new Map<string, number>();
  for (const leg of plan.legs) {
    if (leg.kind === "cross_chain_mint") continue;
    const chain = leg.srcChain ?? "arc";
    totals.set(chain, (totals.get(chain) ?? 0) + leg.amountUsdc);
  }
  return {
    arc: totals.get("arc") ?? 0,
    base: totals.get("base") ?? 0,
  };
}

function chainPositionSaleTotals(plan: RebalancePlanResponse) {
  const totals = new Map<string, number>();
  for (const leg of plan.legs) {
    if (!leg.srcSymbol || leg.srcSymbol === "USDC") continue;
    if (leg.destSymbol !== "USDC") continue;
    const chain = leg.srcChain ?? leg.destChain ?? "arc";
    totals.set(chain, (totals.get(chain) ?? 0) + leg.amountUsdc);
  }
  return {
    arc: totals.get("arc") ?? 0,
    base: totals.get("base") ?? 0,
  };
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
  const [routeOpen, setRouteOpen] = useState(true);

  if (!open || !plan) return null;

  const routedUsdc = routedAmountUsdc(plan);
  const isMockExecution = plan.executionMode === "mock";
  const hasCrossChainLeg = plan.legs.some(isCrossChainLeg);
  const destinations = destinationAmounts(plan);
  const sources = sourceAmounts(plan);
  const bridgedUsdc = bridgedAmountUsdc(plan);
  const bridgeLeg = plan.legs.find((leg) => leg.kind === "cross_chain_burn");
  const bridgeSourceChain = normalizeRouteChain(bridgeLeg?.srcChain ?? "arc");
  const bridgeTargetChain = normalizeRouteChain(bridgeLeg?.destChain ?? "base");
  const hasPositionSales = sources.length > 0;
  const positionSaleUsdc = sources.reduce(
    (acc, source) => acc + source.amountUsdc,
    0,
  );
  const destinationUsdc = destinations.reduce(
    (acc, destination) => acc + destination.amountUsdc,
    0,
  );
  const netTurnoverUsdc = Math.max(positionSaleUsdc, destinationUsdc);
  const approvalBlocked =
    approvalSafety?.approvable === false || isMockExecution;
  const approvalBlockCode = isMockExecution
    ? "HISTORICAL_TEST_REVIEW"
    : (approvalSafety?.code ?? "APPROVAL_BLOCKED");
  const approvalBlockMessage = approvalSafety
    ? blockedReviewMessage(approvalSafety)
    : isMockExecution
      ? "This review was created outside the real execution path. Build a fresh review before approving."
      : "Approval is blocked for this review. Build a fresh review before any execution.";
  const changeHeadline =
    plan.totalLegs === 0
      ? "No portfolio changes needed"
      : hasPositionSales
        ? `Rebalance $${netTurnoverUsdc.toFixed(2)} from overweight positions`
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
                    {isMockExecution ? "Bridge preview" : "Bridge"}{" "}
                    {chainDisplayName(bridgeSourceChain)} →{" "}
                    {chainDisplayName(bridgeTargetChain)}
                  </span>
                  <span>${bridgedUsdc.toFixed(2)}</span>
                </div>
              )}
            </div>
            <p className="mt-3 text-[11px] leading-relaxed text-text-lo">
              {approvalBlocked
                ? blockedAmountCopy(approvalSafety)
                : isMockExecution
                  ? "This historical test review cannot be approved for real execution. Build a fresh review before money moves."
                  : hasPositionSales
                    ? "This approval sells overweight positions, routes USDC, and buys or parks underweight targets. It is not idle-wallet deployment."
                    : "This approval uses wallet USDC for real execution after you confirm."}
            </p>
          </div>

          <RebalanceRouteMap plan={plan} />

          {approvalBlocked && (
            <div className="mb-4 border-brutal border-warn/45 bg-warn/5 p-4 text-xs font-mono text-warn">
              <p className="text-[10px] uppercase tracking-wider">
                {approvalBlockLabel(approvalBlockCode)}
              </p>
              <p className="mt-1 leading-relaxed">{approvalBlockMessage}</p>
              {approvalSafety?.missingCapabilities?.length ? (
                <ul className="mt-3 flex flex-wrap gap-2">
                  {approvalSafety.missingCapabilities.map((capability) => (
                    <li
                      key={capability.code}
                      className="border border-warn/30 bg-black/20 px-2 py-1.5 text-[10px] uppercase tracking-wider"
                    >
                      {capability.label}
                    </li>
                  ))}
                </ul>
              ) : null}
              {approvalSafety ? (
                <BlockedRecoveryActions
                  portfolioId={portfolioId ?? null}
                  safety={approvalSafety}
                />
              ) : null}
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

                  {(() => {
                    const reasoning = displayReasoning(decision);
                    return reasoning ? (
                      <p className="text-text-default leading-relaxed">
                        {reasoning}
                      </p>
                    ) : null;
                  })()}

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
                ? "Historical test route"
                : "Real multi-step execution"}
            </div>
          )}

          <p className="text-sm text-text-default mb-3">
            {approvalBlocked ? (
              <>{blockedLegCopy(plan, approvalSafety)}</>
            ) : (
              <>
                The agent has planned <strong>{plan.totalLegs}</strong> leg
                {plan.totalLegs === 1 ? "" : "s"} to bring your portfolio toward
                its target.{" "}
                {isMockExecution
                  ? "This historical test review is shown for audit only. Build a fresh real-execution review before approving."
                  : hasCrossChainLeg
                    ? "One approval executes the full transfer plan; live updates show each leg as it confirms."
                    : "One approval executes the planned route; live updates show each leg as it confirms."}
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
                        <ChainBadge chain={toChainBadge(leg.srcChain)} />
                      )}
                      {leg.destChain && leg.destChain !== leg.srcChain && (
                        <ChainBadge chain={toChainBadge(leg.destChain)} />
                      )}
                    </span>
                    <span className="text-text-lo">
                      {legRouteText(leg)}
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
              <span>Gross leg notional</span>
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
                  : "Approve & execute"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

function legRouteText(plan: RebalancePlanResponse["legs"][number]) {
  if (plan.kind === "cross_chain_burn") {
    return `${plan.srcSymbol ?? "USDC"} bridge intent → ${plan.destSymbol ?? "destination asset"}`;
  }
  if (plan.kind === "cross_chain_mint") {
    return `Receive bridged ${plan.destSymbol ?? "USDC"} on ${plan.destChain ?? "destination"}`;
  }
  return `${plan.srcSymbol ?? "source"} → ${plan.destSymbol ?? "destination"}`;
}

function toChainBadge(chain: "arc" | "base"): string {
  return walletRouteBadgeLabel(chain);
}

function isCrossChainLeg(plan: RebalancePlanResponse["legs"][number]) {
  return plan.kind === "cross_chain_burn" || plan.kind === "cross_chain_mint";
}

function approvalBlockLabel(code: string): string {
  switch (code) {
    case "HISTORICAL_TEST_REVIEW":
    case "MOCK_OR_LEGACY_PLAN":
      return "Historical test review";
    case "EXECUTION_UNAVAILABLE":
      return "Route not ready";
    case "SUPERSEDED":
      return "Superseded review";
    case "STALE_PLAN":
      return "Stale review";
    case "BALANCE_UNAVAILABLE":
      return "Balance unavailable";
    default:
      return "Approval blocked";
  }
}

function displayReasoning(decision: AgentDecision): string | null {
  const reasoning = decision.reasoning?.trim();
  if (!reasoning) return null;
  if (/mock decision|local\/demo|demo mock mode/i.test(reasoning)) {
    return "This review was generated in demo mode. Build a fresh review to see live strategist commentary.";
  }
  return reasoning;
}

function blockedAmountCopy(safety?: RebalanceApprovalSafety | null): string {
  switch (safety?.code) {
    case "EXECUTION_UNAVAILABLE":
      return "These amounts are current, but one selected route is not ready to move money. Change the target mix, then build a fresh executable review.";
    case "SUPERSEDED":
      return "These amounts belong to an older review. Open the latest review to see the active route.";
    case "STALE_PLAN":
      return "These amounts no longer match current wallet cash or holdings. Build a fresh review before approving.";
    case "BALANCE_UNAVAILABLE":
      return "Wallet cash cannot be verified right now, so real execution stays locked.";
    case "MOCK_OR_LEGACY_PLAN":
      return "This review came from an older non-real planner and cannot be used for real execution.";
    default:
      return "Approval is blocked for this review. Build a fresh review before any execution.";
  }
}

function blockedReviewMessage(safety: RebalanceApprovalSafety): string {
  switch (safety.code) {
    case "EXECUTION_UNAVAILABLE":
      return "This review is saved, but at least one selected route is not ready to move money. Change the target mix, then build a fresh executable review before approving.";
    case "SUPERSEDED":
      return "A newer review exists for this portfolio. Open the latest review or build a fresh one before approving.";
    case "STALE_PLAN":
      return "Wallet cash or holdings changed after this review was created. Build a fresh review so the amounts match current balances.";
    case "BALANCE_UNAVAILABLE":
      return "Wallet cash cannot be verified right now. Check Wallets, then build a fresh review after balances recover.";
    case "MOCK_OR_LEGACY_PLAN":
      return "This review was created outside the current real-execution path. Build a fresh review before approving.";
    default:
      return (
        safety.message ||
        "Approval is blocked for this review. Build a fresh review before any execution."
      );
  }
}

function blockedLegCopy(
  plan: RebalancePlanResponse,
  safety?: RebalanceApprovalSafety | null,
) {
  if (safety?.code === "EXECUTION_UNAVAILABLE") {
    const count = safety.missingCapabilities?.length ?? 0;
    return (
      <>
        Aegis is showing <strong>{plan.totalLegs}</strong> valid review leg
        {plan.totalLegs === 1 ? "" : "s"}, but approval is locked because{" "}
        {count > 1 ? `${count} route checks are` : "one route check is"} not
        ready yet. Change the target mix, then build a fresh executable review.
      </>
    );
  }
  if (safety?.code === "SUPERSEDED" || safety?.code === "STALE_PLAN") {
    return (
      <>
        Aegis is showing these <strong>{plan.totalLegs}</strong> historical leg
        {plan.totalLegs === 1 ? "" : "s"} for audit only. Build a fresh review
        before any execution.
      </>
    );
  }
  return (
    <>
      Aegis is showing <strong>{plan.totalLegs}</strong> blocked leg
      {plan.totalLegs === 1 ? "" : "s"}. Read the block reason above before
      creating the next review.
    </>
  );
}

function BlockedRecoveryActions({
  portfolioId,
  safety,
}: {
  portfolioId: string | null;
  safety: RebalanceApprovalSafety;
}) {
  const dashboardHref = portfolioId
    ? `/dashboard/${portfolioId}`
    : "/dashboard";
  const actions =
    safety.code === "BALANCE_UNAVAILABLE"
      ? [
          {
            href: "/wallets",
            label: "Check wallet cash",
            primary: true,
          },
          {
            href: dashboardHref,
            label: "Build fresh review after balances recover",
            primary: false,
          },
        ]
      : safety.code === "EXECUTION_UNAVAILABLE"
        ? [
            {
              href: dashboardHref,
              label: "Change target mix",
              primary: true,
            },
            {
              href: "/transactions",
              label: "Back to ledger",
              primary: false,
            },
          ]
        : [
            {
              href: dashboardHref,
              label: "Build fresh review",
              primary: true,
            },
            {
              href: "/transactions",
              label: "Back to ledger",
              primary: false,
            },
          ];

  return (
    <div className="mt-3 flex flex-col gap-2 sm:flex-row">
      {actions.map((action) => (
        <a
          key={action.label}
          href={action.href}
          className={
            action.primary
              ? "inline-flex min-h-9 flex-1 items-center justify-center border border-warn/50 bg-warn/10 px-3 py-1.5 text-center text-[11px] font-semibold text-warn hover:bg-warn/15"
              : "inline-flex min-h-9 flex-1 items-center justify-center border border-border-default bg-black/20 px-3 py-1.5 text-center text-[11px] text-text-lo hover:border-border-hi hover:text-text-hi"
          }
        >
          {action.label}
        </a>
      ))}
    </div>
  );
}

function RebalanceRouteMap({ plan }: { plan: RebalancePlanResponse }) {
  const bridged = bridgedAmountUsdc(plan);
  const sourceTotals = chainSourceTotals(plan);
  const saleTotals = chainPositionSaleTotals(plan);
  const targetTotals = chainDestinationTotals(plan);
  const targets = destinationAmounts(plan).slice(0, 4);
  const hasPositionSales = sourceAmounts(plan).length > 0;
  const hasBridge = bridged > 0;
  const bridgeLeg = plan.legs.find((leg) => leg.kind === "cross_chain_burn");
  const sourceChain = normalizeRouteChain(bridgeLeg?.srcChain ?? "arc");
  const targetChain = normalizeRouteChain(bridgeLeg?.destChain ?? "base");
  const sourceUsd = hasPositionSales
    ? chainAmount(saleTotals, sourceChain)
    : chainAmount(sourceTotals, sourceChain);
  const targetUsd = chainAmount(targetTotals, targetChain);

  if (!hasBridge) {
    const chain = normalizeRouteChain(
      plan.legs[0]?.srcChain ?? plan.legs[0]?.destChain ?? "arc",
    );
    return (
      <SingleChainRouteMap
        chain={chain}
        legCount={plan.totalLegs}
        sourceUsd={
          hasPositionSales
            ? chainAmount(saleTotals, chain)
            : chainAmount(sourceTotals, chain)
        }
        targetUsd={chainAmount(targetTotals, chain)}
        targets={targets}
        sourceKind={hasPositionSales ? "positions" : "wallet"}
      />
    );
  }

  return (
    <div className="mb-4 border border-white/10 bg-black/30 p-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
          Execution route
        </p>
        <p className="text-[10px] font-mono text-text-mut">
          {chainLabel(sourceChain)} → {chainLabel(targetChain)} ·{" "}
          {plan.totalLegs} legs
        </p>
      </div>
      <svg
        viewBox="0 0 560 170"
        role="img"
        aria-label={`Route map showing ${chainLabel(sourceChain)} source cash, CCTP bridge, ${chainLabel(targetChain)} target exposure, and target assets`}
        className="h-auto w-full"
      >
        <rect
          x="1"
          y="1"
          width="558"
          height="168"
          fill="#0A0A0A"
          stroke="#2A2A2A"
          strokeWidth="2"
        />
        <g>
          <rect
            x="22"
            y="34"
            width="132"
            height="78"
            fill="#101010"
            stroke={hasPositionSales ? "#fb7185" : "#38E27D"}
            strokeWidth="2"
          />
          <text
            x="38"
            y="61"
            fill={hasPositionSales ? "#fb7185" : "#38E27D"}
            fontFamily="monospace"
            fontSize="12"
            fontWeight="700"
          >
            {chainLabel(sourceChain)} {hasPositionSales ? "SOLD" : "SOURCE"}
          </text>
          <text
            x="38"
            y="86"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="18"
          >
            ${sourceUsd.toFixed(2)}
          </text>
          <text
            x="38"
            y="103"
            fill="#8A8A8A"
            fontFamily="monospace"
            fontSize="10"
          >
            {hasPositionSales ? "positions to USDC" : "source wallet cash"}
          </text>
        </g>

        <path
          d="M158 73H242"
          fill="none"
          stroke={hasBridge ? "#55D7FF" : "#3A3A3A"}
          strokeWidth="3"
          strokeDasharray={hasBridge ? "8 6" : "0"}
        >
          {hasBridge && (
            <animate
              attributeName="stroke-dashoffset"
              from="0"
              to="-28"
              dur="1.5s"
              repeatCount="indefinite"
            />
          )}
        </path>
        <g>
          <rect
            x="232"
            y="45"
            width="96"
            height="56"
            fill="#061318"
            stroke="#55D7FF"
            strokeWidth="2"
          />
          <text
            x="280"
            y="69"
            textAnchor="middle"
            fill="#55D7FF"
            fontFamily="monospace"
            fontSize="11"
            fontWeight="700"
          >
            CCTP V2
          </text>
          <text
            x="280"
            y="88"
            textAnchor="middle"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="13"
          >
            ${bridged.toFixed(2)}
          </text>
        </g>
        <path
          d="M328 73H406"
          fill="none"
          stroke={hasBridge ? "#55D7FF" : "#3A3A3A"}
          strokeWidth="3"
          strokeDasharray={hasBridge ? "8 6" : "0"}
        >
          {hasBridge && (
            <animate
              attributeName="stroke-dashoffset"
              from="0"
              to="-28"
              dur="1.5s"
              repeatCount="indefinite"
            />
          )}
        </path>

        <g>
          <rect
            x="406"
            y="34"
            width="132"
            height="78"
            fill="#101010"
            stroke="#38E27D"
            strokeWidth="2"
          />
          <text
            x="422"
            y="61"
            fill="#38E27D"
            fontFamily="monospace"
            fontSize="12"
            fontWeight="700"
          >
            {chainLabel(targetChain)} TARGET
          </text>
          <text
            x="422"
            y="86"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="18"
          >
            ${targetUsd.toFixed(2)}
          </text>
          <text
            x="422"
            y="103"
            fill="#8A8A8A"
            fontFamily="monospace"
            fontSize="10"
          >
            final exposure
          </text>
        </g>

        <g transform="translate(24 130)">
          {targets.map((target, index) => (
            <g key={target.symbol} transform={`translate(${index * 132} 0)`}>
              <rect
                width="116"
                height="24"
                fill="#151515"
                stroke="#2A2A2A"
                strokeWidth="1"
              />
              <text
                x="9"
                y="16"
                fill="#E8E8E8"
                fontFamily="monospace"
                fontSize="10"
                fontWeight="700"
              >
                {target.symbol}
              </text>
              <text
                x="107"
                y="16"
                textAnchor="end"
                fill="#38E27D"
                fontFamily="monospace"
                fontSize="10"
              >
                ${target.amountUsdc.toFixed(0)}
              </text>
            </g>
          ))}
        </g>
      </svg>
    </div>
  );
}

function SingleChainRouteMap({
  chain,
  legCount,
  sourceUsd,
  targetUsd,
  targets,
  sourceKind,
}: {
  chain: "arc" | "base";
  legCount: number;
  sourceUsd: number;
  targetUsd: number;
  targets: Array<{ symbol: string; amountUsdc: number }>;
  sourceKind: "wallet" | "positions";
}) {
  const sourceStroke = sourceKind === "positions" ? "#fb7185" : "#38E27D";
  return (
    <div className="mb-4 border border-white/10 bg-black/30 p-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
          Execution route
        </p>
        <p className="text-[10px] font-mono text-text-mut">
          single-chain {chainLabel(chain)} · {legCount} legs
        </p>
      </div>
      <svg
        viewBox="0 0 560 170"
        role="img"
        aria-label={`Route map showing ${chainLabel(chain)} wallet USDC flowing into target sleeves without a CCTP bridge`}
        className="h-auto w-full"
      >
        <rect
          x="1"
          y="1"
          width="558"
          height="168"
          fill="#0A0A0A"
          stroke="#2A2A2A"
          strokeWidth="2"
        />
        <g>
          <rect
            x="24"
            y="34"
            width="150"
            height="78"
            fill="#101010"
            stroke={sourceStroke}
            strokeWidth="2"
          />
          <text
            x="42"
            y="61"
            fill={sourceStroke}
            fontFamily="monospace"
            fontSize="12"
            fontWeight="700"
          >
            {chainLabel(chain)} {sourceKind === "positions" ? "SOLD" : "WALLET"}
          </text>
          <text
            x="42"
            y="86"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="18"
          >
            ${sourceUsd.toFixed(2)}
          </text>
          <text
            x="42"
            y="103"
            fill="#8A8A8A"
            fontFamily="monospace"
            fontSize="10"
          >
            {sourceKind === "positions" ? "positions to USDC" : "USDC used now"}
          </text>
        </g>

        <path
          d="M178 73H322"
          fill="none"
          stroke="#38E27D"
          strokeWidth="3"
          strokeDasharray="9 7"
        >
          <animate
            attributeName="stroke-dashoffset"
            from="0"
            to="-32"
            dur="1.4s"
            repeatCount="indefinite"
          />
        </path>
        <g>
          <rect
            x="312"
            y="34"
            width="224"
            height="78"
            fill="#101010"
            stroke="#38E27D"
            strokeWidth="2"
          />
          <text
            x="332"
            y="61"
            fill="#38E27D"
            fontFamily="monospace"
            fontSize="12"
            fontWeight="700"
          >
            TARGET SLEEVES
          </text>
          <text
            x="332"
            y="86"
            fill="#E8E8E8"
            fontFamily="monospace"
            fontSize="18"
          >
            ${targetUsd.toFixed(2)}
          </text>
          <text
            x="332"
            y="103"
            fill="#8A8A8A"
            fontFamily="monospace"
            fontSize="10"
          >
            no bridge needed
          </text>
        </g>

        <g transform="translate(24 130)">
          {targets.map((target, index) => (
            <g key={target.symbol} transform={`translate(${index * 132} 0)`}>
              <rect
                width="116"
                height="24"
                fill="#151515"
                stroke="#2A2A2A"
                strokeWidth="1"
              />
              <text
                x="9"
                y="16"
                fill="#E8E8E8"
                fontFamily="monospace"
                fontSize="10"
                fontWeight="700"
              >
                {target.symbol}
              </text>
              <text
                x="107"
                y="16"
                textAnchor="end"
                fill="#38E27D"
                fontFamily="monospace"
                fontSize="10"
              >
                ${target.amountUsdc.toFixed(0)}
              </text>
            </g>
          ))}
          <g transform={`translate(${Math.min(targets.length, 3) * 132} 0)`}>
            <rect
              width="128"
              height="24"
              fill="#111A14"
              stroke="#38E27D"
              strokeWidth="1"
            />
            <text
              x="9"
              y="16"
              fill="#38E27D"
              fontFamily="monospace"
              fontSize="10"
              fontWeight="700"
            >
              USDC RESERVE
            </text>
          </g>
        </g>
      </svg>
    </div>
  );
}

function normalizeRouteChain(chain: string): "arc" | "base" {
  return chain.toLowerCase() === "base" ? "base" : "arc";
}

function chainAmount(
  totals: { arc: number; base: number },
  chain: "arc" | "base",
) {
  return chain === "base" ? totals.base : totals.arc;
}

function chainLabel(chain: "arc" | "base") {
  return chain.toUpperCase();
}

function chainDisplayName(chain: "arc" | "base") {
  return chain === "base" ? "Base" : "Arc";
}
