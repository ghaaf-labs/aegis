import type { WalletNetwork } from "@/types";
import type { ExplorerChain } from "./explorers";

export interface WalletRouteMeta {
  key: ExplorerChain;
  blockchain: string;
  label: string;
  shortLabel: string;
  executionReady: boolean;
}

export interface ChainBalanceRow {
  key: ExplorerChain;
  label: string;
  shortLabel: string;
  usdc: number;
  eurc: number;
  totalUsd: number;
}

export const WALLET_ROUTES: WalletRouteMeta[] = [
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

export function normalizeWalletRouteKey(
  key: string | null | undefined,
): ExplorerChain | null {
  if (!key) return null;
  const normalized = key.trim().toLowerCase().replaceAll("_", "-");
  if (ROUTES_BY_KEY.has(normalized as ExplorerChain)) {
    return normalized as ExplorerChain;
  }
  return null;
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
