import {
  type RebalanceApprovalSafety,
  type RebalancePlanResponse,
} from "@/lib/api";
import type { AgentDecision } from "@/types";
import { walletRouteBadgeLabel } from "@/lib/wallet-routes";

export const KIND_LABEL: Record<string, string> = {
  local_swap: "Swap",
  cross_chain_burn: "CCTP burn",
  cross_chain_mint: "CCTP mint + hook",
  park_usyc: "Park → USYC",
  redeem_usyc: "Redeem ← USYC",
  fx_stablefx: "StableFX",
};

/** Headline confidence the modal renders.
 *
 * Prefers the histogram-bin calibrated confidence (F-CONF-4 → agent service
 * with CALIBRATED_CONF_ENABLED=true). Falls back to the strategist's flat
 * raw confidence when no calibration exists yet, then to the legacy
 * `confidence` field for back-compat with decisions persisted before
 * migration 0013. */
export function pickHeadlineConfidence(decision: AgentDecision): number {
  if (typeof decision.calibratedConfidence === "number") {
    return decision.calibratedConfidence;
  }
  if (typeof decision.rawConfidence === "number") {
    return decision.rawConfidence;
  }
  return decision.confidence ?? 0;
}

export function formatRelativeSeconds(at: Date): string {
  const secs = Math.max(0, Math.round((Date.now() - at.getTime()) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.round(secs / 60)}m ago`;
  return `${Math.round(secs / 3600)}h ago`;
}

export function routedAmountUsdc(plan: RebalancePlanResponse): number {
  // Count only terminal acquisitions (swaps / USYC parks), NOT the CCTP bridge
  // legs: a `cross_chain_burn` relocates USDC that a later swap leg then spends,
  // so counting both double-counts the bridged amount — it inflated the
  // "Deploy $X" headline past the real "final exposure" (e.g. $784 vs $412 when
  // $372 was bridged then swapped). Excluding both bridge legs makes the
  // headline equal the money actually deployed into assets.
  return plan.legs
    .filter(
      (leg) =>
        leg.kind !== "cross_chain_mint" && leg.kind !== "cross_chain_burn",
    )
    .reduce((acc, leg) => acc + leg.amountUsdc, 0);
}

export function destinationAmounts(plan: RebalancePlanResponse): Array<{
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

export function sourceAmounts(plan: RebalancePlanResponse): Array<{
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

export function bridgedAmountUsdc(plan: RebalancePlanResponse): number {
  return plan.legs
    .filter((leg) => leg.kind === "cross_chain_burn")
    .reduce((acc, leg) => acc + leg.amountUsdc, 0);
}

export function chainDestinationTotals(plan: RebalancePlanResponse) {
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

export function chainSourceTotals(plan: RebalancePlanResponse) {
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

export function chainPositionSaleTotals(plan: RebalancePlanResponse) {
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

export function sourceActionLabel(symbol: string) {
  return symbol === "USYC" ? "Redeem USYC" : `Sell ${symbol}`;
}

export function destinationActionLabel(symbol: string) {
  return symbol === "USYC" ? "Move to USYC" : `Buy ${symbol}`;
}

export function legRouteText(plan: RebalancePlanResponse["legs"][number]) {
  if (plan.kind === "cross_chain_burn") {
    return `${plan.srcSymbol ?? "USDC"} bridge intent → ${plan.destSymbol ?? "destination asset"}`;
  }
  if (plan.kind === "cross_chain_mint") {
    return `Receive bridged ${plan.destSymbol ?? "USDC"} on ${plan.destChain ?? "destination"}`;
  }
  return `${plan.srcSymbol ?? "source"} → ${plan.destSymbol ?? "destination"}`;
}

export function toChainBadge(chain: "arc" | "base"): string {
  return walletRouteBadgeLabel(chain);
}

export function isCrossChainLeg(plan: RebalancePlanResponse["legs"][number]) {
  return plan.kind === "cross_chain_burn" || plan.kind === "cross_chain_mint";
}

export function approvalBlockLabel(code: string): string {
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
      return "Needs changes";
  }
}

export function displayReasoning(decision: AgentDecision): string | null {
  const reasoning = decision.reasoning?.trim();
  if (!reasoning) return null;
  if (/mock decision|local\/demo|demo mock mode/i.test(reasoning)) {
    return "This review was generated in demo mode. Build a fresh review to see live strategist commentary.";
  }
  return reasoning;
}

export function blockedReviewMessage(safety: RebalanceApprovalSafety): string {
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
        "Approval needs changes for this review. Build a fresh review before any execution."
      );
  }
}

export function blockedLegCopy(
  plan: RebalancePlanResponse,
  safety?: RebalanceApprovalSafety | null,
) {
  if (safety?.code === "EXECUTION_UNAVAILABLE") {
    const count = safety.missingCapabilities?.length ?? 0;
    return (
      <>
        Aegis is showing <strong>{plan.totalLegs}</strong> valid review leg
        {plan.totalLegs === 1 ? "" : "s"}, but approval is paused because{" "}
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
      Aegis is showing <strong>{plan.totalLegs}</strong> review leg
      {plan.totalLegs === 1 ? "" : "s"} that need changes. Read the reason above
      before creating the next review.
    </>
  );
}

export function normalizeRouteChain(chain: string): "arc" | "base" {
  return chain.toLowerCase() === "base" ? "base" : "arc";
}

export function chainAmount(
  totals: { arc: number; base: number },
  chain: "arc" | "base",
) {
  return chain === "base" ? totals.base : totals.arc;
}

export function chainLabel(chain: "arc" | "base") {
  return chain.toUpperCase();
}

export function chainDisplayName(chain: "arc" | "base") {
  return chain === "base" ? "Base" : "Arc";
}
