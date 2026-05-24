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

export const TOKEN_ROUTE_OPTIONS = [
  {
    id: "USDC",
    symbol: "USDC",
    label: "Cash",
    state: "Ready",
    detail: "Reserve, funding, and transfer route is ready",
    executable: true,
    targetable: true,
  },
  {
    id: "BTC",
    symbol: "BTC",
    label: "Market target",
    state: "Track only",
    detail: "Price tracking is ready; swap execution is not connected yet",
    executable: false,
    targetable: true,
  },
  {
    id: "ETH",
    symbol: "ETH",
    label: "Market target",
    state: "Track only",
    detail: "Price tracking is ready; swap execution is not connected yet",
    executable: false,
    targetable: true,
  },
  {
    id: "SOL",
    symbol: "SOL",
    label: "Market target",
    state: "Track only",
    detail: "Price tracking is ready; swap execution is not connected yet",
    executable: false,
    targetable: true,
  },
  {
    id: "USYC",
    symbol: "USYC",
    label: "Yield target",
    state: "Coming soon",
    detail:
      "Not available in this build; USYC stays visible as a coming-soon route",
    executable: false,
    targetable: false,
  },
  {
    id: "EURC",
    symbol: "EURC",
    label: "FX target",
    state: "Track only",
    detail:
      "FX tracking is ready; EURC executes on the Base USDC/EURC pool when the swap rail is live",
    executable: false,
    targetable: true,
  },
] as const;

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
