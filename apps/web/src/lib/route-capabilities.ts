import { TOKENS, type RouteReadiness } from "@aegis/shared";

import { VOLATILE_EXECUTION_ENABLED } from "@/lib/flags";

export const COMING_SOON_TOKEN_IDS = TOKENS.filter(
  (token) => token.designable && token.comingSoon,
).map((token) => token.symbol);

// A sleeve the planner can actually trade/rebalance on this deployment, mirroring
// the backend `rebalanceable_token_symbols` / `is_volatile_sleeve` split:
//   • stablecoins are always tradeable (1:1, no AMM price gap);
//   • *volatile* sleeves are tracked-not-traded until the deployment turns on
//     volatile execution (`VOLATILE_EXECUTION_ENABLED`), which mainnet does;
//   • FX/yield sleeves (EURC, USYC) are gated by their own rails (StableFX KYB,
//     USYC_ENABLED) — the volatile flag does not make them tradeable, so this
//     coarse predicate keeps them tracked. Exact per-asset readiness for those
//     comes from backend `routeStates` (`allocationDisplayMeta`).
// Used for route badges *and* for honest drift — drift in a tracked sleeve is
// not reviewable because the user cannot action it here.
export function isTradeableSleeve(tokenId: string): boolean {
  const token = TOKENS.find((candidate) => candidate.symbol === tokenId);
  if (token?.designable !== true || token.comingSoon === true) {
    return false;
  }
  if (token.class === "stable") {
    return true;
  }
  return token.class === "volatile" && VOLATILE_EXECUTION_ENABLED;
}

function tokenComingSoon(tokenId: string): boolean {
  const token = TOKENS.find((candidate) => candidate.symbol === tokenId);
  return token?.designable === true && token.comingSoon === true;
}

// Friendly display names for proposed sleeves, derived from the shared `TOKENS`
// table (the FE projection of the backend registry, guarded by the Rust test
// `fe_token_contract_matches_generated_json`) so they can never drift: the modal
// shows "Bitcoin" rather than the execution symbol "cbBTC".
const TOKEN_FRIENDLY_LABELS: Record<string, string> = Object.fromEntries(
  TOKENS.map((token) => [token.symbol, token.label]),
);

export type RouteStateLabel =
  | "executes-now"
  | "target-pending-rail"
  | "track-only"
  | "coming-soon";

export interface AllocationDisplayMeta {
  label: string;
  routeState: RouteStateLabel;
  badge: string;
}

// Resolve a proposed allocation symbol to a friendly label and an honest
// per-asset route state for the proposal modal. When the backend supplies the
// live `readiness` for this decision (the route engine's verdict for the
// current deployment) it is authoritative. Legacy/queued decisions fall back to
// cash as executable, coming-soon sleeves as gated, and every other designable
// sleeve as an honest pending target.
export function allocationDisplayMeta(
  symbol: string,
  readiness?: RouteReadiness,
): AllocationDisplayMeta {
  const label = TOKEN_FRIENDLY_LABELS[symbol] ?? symbol;
  if (readiness !== undefined) {
    if (readiness === "ready") {
      return { label, routeState: "executes-now", badge: "Executes now" };
    }
    if (readiness === "track-only") {
      return { label, routeState: "track-only", badge: "Track only" };
    }
    return {
      label,
      routeState: "target-pending-rail",
      badge: "Executes when rail live",
    };
  }
  if (isTradeableSleeve(symbol)) {
    return { label, routeState: "executes-now", badge: "Executes now" };
  }
  if (tokenComingSoon(symbol)) {
    return { label, routeState: "coming-soon", badge: "Coming soon" };
  }
  return {
    label,
    routeState: "target-pending-rail",
    badge: "Executes when rail live",
  };
}
