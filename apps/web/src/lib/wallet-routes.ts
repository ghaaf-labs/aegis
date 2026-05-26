import type { WalletNetwork } from "@/types";
import type { ExplorerChain } from "./explorers";

interface WalletRouteMeta {
  key: ExplorerChain;
  blockchain: string;
  label: string;
  shortLabel: string;
  executionReady: boolean;
}

interface ChainBalanceRow {
  key: ExplorerChain;
  label: string;
  shortLabel: string;
  usdc: number;
  eurc: number;
  totalUsd: number;
}

const WALLET_ROUTES: WalletRouteMeta[] = [
  {
    key: "arc",
    blockchain: "ARC-TESTNET",
    label: "Arc testnet",
    shortLabel: "Arc",
    executionReady: true,
  },
  {
    key: "base",
    blockchain: "BASE-SEPOLIA",
    label: "Base Sepolia",
    shortLabel: "Base",
    executionReady: true,
  },
  {
    key: "eth-sepolia",
    blockchain: "ETH-SEPOLIA",
    label: "Ethereum Sepolia",
    shortLabel: "Ethereum",
    executionReady: false,
  },
  {
    key: "arb-sepolia",
    blockchain: "ARB-SEPOLIA",
    label: "Arbitrum Sepolia",
    shortLabel: "Arbitrum",
    executionReady: false,
  },
  {
    key: "avax-fuji",
    blockchain: "AVAX-FUJI",
    label: "Avalanche Fuji",
    shortLabel: "Avalanche",
    executionReady: false,
  },
];

const ROUTES_BY_KEY = new Map(WALLET_ROUTES.map((route) => [route.key, route]));

const BLOCKCHAIN_ALIASES = new Map<string, ExplorerChain>([
  ["ARC", "arc"],
  ["ARC-TESTNET", "arc"],
  ["BASE", "base"],
  ["BASE-SEPOLIA", "base"],
  ["ETH", "eth-sepolia"],
  ["ETH-SEPOLIA", "eth-sepolia"],
  ["ETHEREUM-SEPOLIA", "eth-sepolia"],
  ["ARB", "arb-sepolia"],
  ["ARB-SEPOLIA", "arb-sepolia"],
  ["ARBITRUM-SEPOLIA", "arb-sepolia"],
  ["AVAX", "avax-fuji"],
  ["AVAX-FUJI", "avax-fuji"],
  ["AVALANCHE-FUJI", "avax-fuji"],
]);

export function walletRouteFromBlockchain(
  blockchain: string | null | undefined,
) {
  const key = walletRouteKeyFromBlockchain(blockchain);
  return key ? (ROUTES_BY_KEY.get(key) ?? null) : null;
}

export function walletRouteFromKey(key: string | null | undefined) {
  const normalized = normalizeWalletRouteKey(key);
  return normalized ? (ROUTES_BY_KEY.get(normalized) ?? null) : null;
}

export function walletRouteKeyFromBlockchain(
  blockchain: string | null | undefined,
): ExplorerChain | null {
  if (!blockchain) return null;
  const normalized = blockchain.trim().toUpperCase().replaceAll("_", "-");
  return (
    BLOCKCHAIN_ALIASES.get(normalized) ?? normalizeWalletRouteKey(blockchain)
  );
}

function normalizeWalletRouteKey(
  key: string | null | undefined,
): ExplorerChain | null {
  if (!key) return null;
  const normalized = key.trim().toLowerCase().replaceAll("_", "-");
  if (ROUTES_BY_KEY.has(normalized as ExplorerChain)) {
    return normalized as ExplorerChain;
  }
  return null;
}

// Minimum idle USDC on a non-primary chain worth bridging. Mirrors the backend
// routing engine's `CONSOLIDATION_MIN_USD` — below this the cash stays put.
// Module-private: callers consume the `idleUsdcConsolidation` predicate, which
// is the single shared entry point, rather than re-deriving the rule.
const CONSOLIDATION_MIN_USD = 5;

export interface IdleUsdcConsolidation {
  /** Non-primary chains holding consolidatable idle USDC (the bridge sources). */
  sources: number;
  /** Chains holding consolidatable idle USDC, including the primary. */
  fundedChains: number;
}

/**
 * Mirror of the backend routing engine's idle-USDC consolidation rule
 * (`apps/api/src/modules/rebalance/routing/mod.rs` → `append_consolidation_legs`):
 * idle USDC stranded on any execution chain other than the *primary* (the
 * Arc/Base chain holding the most idle USDC, defaulting to Base on a tie) is
 * swept onto that primary over CCTP when the stranded amount clears
 * {@link CONSOLIDATION_MIN_USD}. Kept beside the chain metadata so the
 * dashboard's "Consolidate idle USDC" hint surfaces a card iff the backend would
 * actually plan a leg — including the single-non-primary-chain case the old
 * "2+ funded chains" heuristic missed.
 */
export function idleUsdcConsolidation(
  perChainUsdc: Record<string, number>,
): IdleUsdcConsolidation {
  const byKey = new Map<ExplorerChain, number>();
  for (const [rawKey, amount] of Object.entries(perChainUsdc)) {
    const key = walletRouteKeyFromBlockchain(rawKey);
    if (!key || !Number.isFinite(amount) || amount <= 0) continue;
    byKey.set(key, (byKey.get(key) ?? 0) + amount);
  }
  // Primary picks the richer Arc/Base chain from *all* idle (matching the
  // backend), ties → Base; the source/funded counts then apply the threshold.
  const primary: ExplorerChain =
    (byKey.get("arc") ?? 0) > (byKey.get("base") ?? 0) ? "arc" : "base";
  let sources = 0;
  let fundedChains = 0;
  for (const [key, amount] of byKey) {
    if (amount < CONSOLIDATION_MIN_USD) continue;
    fundedChains += 1;
    if (key !== primary) sources += 1;
  }
  return { sources, fundedChains };
}

export function walletRouteKeysFromNetworks(
  networks: Pick<WalletNetwork, "blockchain">[] | null | undefined,
): ExplorerChain[] {
  const keys: ExplorerChain[] = [];
  for (const network of networks ?? []) {
    const key = walletRouteKeyFromBlockchain(network.blockchain);
    if (key && !keys.includes(key)) {
      keys.push(key);
    }
  }
  return keys;
}

export function walletRouteLabel(blockchain: string) {
  return (
    walletRouteFromBlockchain(blockchain)?.label ??
    blockchain
      .replaceAll("-", " ")
      .toLowerCase()
      .replace(/\b\w/g, (letter) => letter.toUpperCase())
  );
}

export function walletRouteBadgeLabel(chain: string) {
  return (
    walletRouteFromKey(chain)?.shortLabel ??
    walletRouteFromBlockchain(chain)?.shortLabel ??
    chain
  ).toUpperCase();
}

export function chainBalanceRows({
  perChainUsdc,
  perChainEurc,
  eurcUsd,
  routeKeys,
}: {
  perChainUsdc: Record<string, number>;
  perChainEurc: Record<string, number>;
  eurcUsd: number;
  routeKeys?: ExplorerChain[];
}): ChainBalanceRow[] {
  const preferred = routeKeys && routeKeys.length > 0 ? routeKeys : [];
  const balanceKeys = [
    ...Object.keys(perChainUsdc),
    ...Object.keys(perChainEurc),
  ].flatMap((key) => {
    const normalized = walletRouteKeyFromBlockchain(key);
    return normalized ? [normalized] : [];
  });
  const keys = uniqueRoutes([...preferred, ...balanceKeys]);
  const rowsFor: ExplorerChain[] = keys.length > 0 ? keys : ["arc", "base"];

  return rowsFor.map((key) => {
    const route = ROUTES_BY_KEY.get(key)!;
    const usdc = amountForRoute(perChainUsdc, route);
    const eurc = amountForRoute(perChainEurc, route);
    return {
      key,
      label: route.label,
      shortLabel: route.shortLabel,
      usdc,
      eurc,
      totalUsd: usdc + eurc * eurcUsd,
    };
  });
}

function uniqueRoutes(keys: ExplorerChain[]) {
  const seen = new Set<ExplorerChain>();
  const ordered: ExplorerChain[] = [];
  for (const key of keys) {
    if (!seen.has(key)) {
      seen.add(key);
      ordered.push(key);
    }
  }
  return ordered;
}

function amountForRoute(
  values: Record<string, number>,
  route: WalletRouteMeta,
) {
  const candidates = [
    route.key,
    route.blockchain,
    route.blockchain.toLowerCase(),
    route.blockchain.toLowerCase().replaceAll("-", "_"),
  ];
  for (const candidate of candidates) {
    const value = values[candidate];
    if (typeof value === "number") {
      return value;
    }
  }
  return 0;
}
