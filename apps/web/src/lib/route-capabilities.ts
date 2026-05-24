import { TOKENS, type RouteReadiness } from "@aegis/shared";

// Chains with a full local execution venue (hold + swap + bridge). Eth/Arb/Avax
// are provisioned and CCTP-bridgeable — their USDC IS used in rebalances, just
// auto-consolidated to one of these — but they have no local swap venue, so they
// are not "venue ready". Internal to `executionReady`; not a public export.
const EXECUTION_NETWORK_IDS = ["ARC-TESTNET", "BASE-SEPOLIA"] as const;

export const NETWORK_ROUTE_OPTIONS = [
  {
    blockchain: "ARC-TESTNET",
    label: "Arc testnet",
    detail: "Wallet address and rebalance rail are ready",
    executionReady: true,
  },
  {
    blockchain: "BASE-SEPOLIA",
    label: "Base Sepolia",
    detail: "Wallet address and rebalance rail are ready",
    executionReady: true,
  },
  {
    blockchain: "ETH-SEPOLIA",
    label: "Ethereum Sepolia",
    detail: "Synced; USDC here is auto-consolidated to Arc/Base for rebalances",
    executionReady: false,
  },
  {
    blockchain: "ARB-SEPOLIA",
    label: "Arbitrum Sepolia",
    detail: "Synced; USDC here is auto-consolidated to Arc/Base for rebalances",
    executionReady: false,
  },
  {
    blockchain: "AVAX-FUJI",
    label: "Avalanche Fuji",
    detail: "Synced; USDC here is auto-consolidated to Arc/Base for rebalances",
    executionReady: false,
  },
] as const;

export interface RouteOption {
  id: string;
  symbol: string;
  /** Route-category label: Cash / Market target / FX target / Yield target. */
  label: string;
  /** UI readiness: Ready / Track only / Coming soon. */
  state: string;
  detail: string;
  /** Rail is live today (cash only — execution-liveness is runtime). */
  executable: boolean;
  /** May be assigned a target weight (every designable, non-coming-soon sleeve). */
  targetable: boolean;
}

// The user-facing token picker, DERIVED from the shared `TOKENS` table (the
// backend registry projection, guarded by the Rust drift test) so it can never
// drift: every designable sleeve appears, and its route-readiness presentation
// is a pure function of the token's class + coming-soon gate. Execution-liveness
// is runtime, so only cash (USDC) reads as "executable"; the rest are honest
// track-only targets until the swap rail is live.
export const TOKEN_ROUTE_OPTIONS: RouteOption[] = TOKENS.filter(
  (token) => token.designable,
).map((token) => {
  const executable = token.class === "stable" && !token.comingSoon;
  const targetable = !token.comingSoon;
  const label =
    token.class === "stable"
      ? "Cash"
      : token.class === "fx_stable"
        ? "FX target"
        : token.class === "yield"
          ? "Yield target"
          : "Market target";
  const state = token.comingSoon
    ? "Coming soon"
    : executable
      ? "Ready"
      : "Track only";
  const detail = token.comingSoon
    ? "Not available in this build; USYC stays visible as a coming-soon route"
    : executable
      ? "Reserve, funding, and transfer route is ready"
      : token.class === "fx_stable"
        ? "FX tracking is ready; EURC executes on the Base USDC/EURC pool when the swap rail is live"
        : "Price tracking is ready; swap execution is not connected yet";
  return {
    id: token.symbol,
    symbol: token.symbol,
    label,
    state,
    detail,
    executable,
    targetable,
  };
});

export const TARGET_TOKEN_IDS = TOKEN_ROUTE_OPTIONS.filter(
  (token) => token.targetable,
).map((token) => token.id);
export const EXECUTABLE_TOKEN_IDS = TOKEN_ROUTE_OPTIONS.filter(
  (token) => token.executable,
).map((token) => token.id);
export const TRACK_ONLY_TOKEN_IDS = TOKEN_ROUTE_OPTIONS.filter(
  (token) => !token.executable && token.targetable,
).map((token) => token.id);
export const COMING_SOON_TOKEN_IDS = TOKEN_ROUTE_OPTIONS.filter(
  (token) => !token.targetable,
).map((token) => token.id);

export function executionReady(blockchain: string): boolean {
  return EXECUTION_NETWORK_IDS.includes(
    blockchain as (typeof EXECUTION_NETWORK_IDS)[number],
  );
}

export function tokenExecutable(tokenId: string): boolean {
  return TOKEN_ROUTE_OPTIONS.some(
    (token) => token.id === tokenId && token.executable,
  );
}

export function tokenTargetable(tokenId: string): boolean {
  return TOKEN_ROUTE_OPTIONS.some(
    (token) => token.id === tokenId && token.targetable,
  );
}

function tokenComingSoon(tokenId: string): boolean {
  return TOKEN_ROUTE_OPTIONS.some(
    (token) => token.id === tokenId && !token.targetable,
  );
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
// current deployment) it is authoritative; otherwise we fall back to the static
// route table (legacy/queued decisions). Cash and the executable risk sleeves
// read "Executes now"; sleeves with no liquid rail are honest "Track only"
// targets (never silently dropped or relabelled as USDC).
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
