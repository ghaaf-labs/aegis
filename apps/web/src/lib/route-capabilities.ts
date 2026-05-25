import { TOKENS, type RouteReadiness } from "@aegis/shared";

export const COMING_SOON_TOKEN_IDS = TOKENS.filter(
  (token) => token.designable && token.comingSoon,
).map((token) => token.symbol);

function tokenExecutable(tokenId: string): boolean {
  const token = TOKENS.find((candidate) => candidate.symbol === tokenId);
  return (
    token?.designable === true && token.class === "stable" && !token.comingSoon
  );
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
  if (tokenExecutable(symbol)) {
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
