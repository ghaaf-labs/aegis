import type {
  AssetSymbol,
  MarketSnapshot,
  Portfolio,
  WalletInfo,
} from "@/types";
import { deriveCashSplit } from "@/lib/cash-model";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { targetAllocationsForPortfolio } from "@/lib/target-allocations";
import { isTradeableSleeve } from "@/lib/route-capabilities";
import {
  chainBalanceRows,
  walletRouteKeysFromNetworks,
} from "@/lib/wallet-routes";

type BalanceStatus = "idle" | "loading" | "ready" | "error";

/**
 * Drift the user can actually action on this network. Only tradeable sleeves can
 * be rebalanced (volatiles are tracked-not-traded here), so drift is measured
 * over them and renormalized to the tradeable base — comparing a tradeable
 * sleeve's share of total net worth against a target that also spans tracked
 * sleeves would report phantom drift the user can never close. A tracked-heavy
 * portfolio whose tradeable slice (USDC) is on target reads as on target.
 */
function reviewableDriftPct(
  tokens: { symbol: string; totalUsd: number; targetWeight: number }[],
): number {
  const tradeable = tokens.filter((token) => isTradeableSleeve(token.symbol));
  const base = tradeable.reduce((sum, token) => sum + token.totalUsd, 0);
  const targetSum = tradeable.reduce(
    (sum, token) => sum + token.targetWeight,
    0,
  );
  if (base <= 0.005 || targetSum <= 0) {
    return 0;
  }
  return tradeable.reduce((max, token) => {
    const actualShare = (token.totalUsd / base) * 100;
    const targetShare = (token.targetWeight / targetSum) * 100;
    return Math.max(max, Math.abs(actualShare - targetShare));
  }, 0);
}

export interface DashboardBalanceInput {
  portfolio: Portfolio | null | undefined;
  snapshot: MarketSnapshot | null | undefined;
  wallet: WalletInfo | null | undefined;
  unifiedUsdc: number;
  unifiedEurc: number;
  perChainUsdc: Record<string, number>;
  perChainEurc: Record<string, number>;
  gatewayBalanceStatus: BalanceStatus;
  gatewayBalanceError: string | null;
  gatewayBalanceUpdatedAt: number | null;
  extraTokenBalancesByChain?: Record<string, Record<string, number>>;
}

export interface DashboardTokenExposure {
  symbol: AssetSymbol | string;
  walletUsd: number;
  investedUsd: number;
  totalUsd: number;
  targetWeight: number;
  weightPct: number;
}

export interface DashboardChainExposure {
  key: string;
  label: string;
  shortLabel: string;
  totalUsd: number;
  weightPct: number;
}

export interface DashboardMatrixCell {
  chainKey: string;
  valueUsd: number;
  shareOfWalletPct: number;
}

interface DashboardMatrixRow {
  symbol: AssetSymbol | string;
  cells: DashboardMatrixCell[];
  totalUsd: number;
  weightPct: number;
}

export interface DashboardBalanceModel {
  netWorthUsd: number;
  investedUsd: number;
  walletValueUsd: number;
  reserveUsd: number;
  reservePct: number;
  deployableUsd: number;
  unifiedUsdc: number;
  unifiedEurc: number;
  eurcUsd: number;
  hasIdleCash: boolean;
  hasAgentTarget: boolean;
  hasInvestedPositions: boolean;
  hasReviewableDrift: boolean;
  maxTargetDriftPct: number;
  walletBalanceLoading: boolean;
  walletBalanceUnavailable: boolean;
  gatewayBalanceError: string | null;
  gatewayBalanceUpdatedAt: number | null;
  status: DashboardStatus;
  /** Idle USDC per chain (Circle Gateway) — drives consolidation detection. */
  perChainUsdc: Record<string, number>;
  tokens: DashboardTokenExposure[];
  chains: DashboardChainExposure[];
  matrixRows: DashboardMatrixRow[];
  matrixTotals: DashboardMatrixCell[];
  matrixTotalUsd: number;
  tokenCount: number;
  chainCount: number;
  addressCount: number;
}

interface DashboardStatus {
  label: string;
  detail: string;
  tone: "pnl" | "agent" | "warn" | "risk" | "muted";
}

const USDC = "USDC";
const EURC = "EURC";
const USYC = "USYC";

export function deriveDashboardBalanceModel({
  portfolio,
  snapshot,
  wallet,
  unifiedUsdc,
  unifiedEurc,
  perChainUsdc,
  perChainEurc,
  gatewayBalanceStatus,
  gatewayBalanceError,
  gatewayBalanceUpdatedAt,
  extraTokenBalancesByChain = {},
}: DashboardBalanceInput): DashboardBalanceModel {
  const targetAllocations = targetAllocationsForPortfolio(portfolio);
  const positionMetrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const preliminaryCashSplit = deriveCashSplit({
    unifiedUsdc,
    unifiedEurc,
    targetAllocations,
    investedUsd: positionMetrics.investedUsd,
    snapshot,
  });

  const walletBalanceLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const walletBalanceUnavailable = gatewayBalanceStatus === "error";
  const useLiveWalletTokenBalances = gatewayBalanceStatus === "ready";
  const routeRows = chainBalanceRows({
    perChainUsdc,
    perChainEurc,
    eurcUsd: preliminaryCashSplit.eurcUsd,
    routeKeys: walletRouteKeysFromNetworks(wallet?.networks),
  });
  const cashMatrixTotalUsd = routeRows.reduce(
    (sum, row) => sum + row.usdc + row.eurc * preliminaryCashSplit.eurcUsd,
    0,
  );
  const routeKeys: string[] = routeRows.map((row) => row.key);
  const walletByTokenByChain = new Map<string, Map<string, number>>();
  let liveInvestedUsd = 0;
  const liveSymbols = new Set<string>();

  for (const row of routeRows) {
    addWalletTokenValue(walletByTokenByChain, USDC, row.key, row.usdc);
    addWalletTokenValue(
      walletByTokenByChain,
      EURC,
      row.key,
      row.eurc * preliminaryCashSplit.eurcUsd,
    );
  }

  for (const [chainKey, balances] of Object.entries(
    extraTokenBalancesByChain,
  )) {
    for (const [symbol, quantity] of Object.entries(balances)) {
      if (!Number.isFinite(quantity) || quantity <= 0) continue;
      const markPriceUsd = priceUsdForSymbol(
        symbol,
        snapshot,
        preliminaryCashSplit.eurcUsd,
      );
      const valueUsd = quantity * markPriceUsd;
      liveInvestedUsd += valueUsd;
      if (valueUsd > 0.005) liveSymbols.add(symbol);
      addWalletTokenValue(walletByTokenByChain, symbol, chainKey, valueUsd);
      if (!routeKeys.includes(chainKey)) {
        routeKeys.push(chainKey);
      }
    }
  }
  if (!useLiveWalletTokenBalances) {
    for (const position of positionMetrics.positions) {
      if (
        position.valueUsd <= 0.005 ||
        liveSymbols.has(position.symbol) ||
        isCashSymbol(position.symbol)
      ) {
        continue;
      }
      const fallbackChain = "base";
      addWalletTokenValue(
        walletByTokenByChain,
        position.symbol,
        fallbackChain,
        position.valueUsd,
      );
      liveInvestedUsd += position.valueUsd;
      liveSymbols.add(position.symbol);
      if (!routeKeys.includes(fallbackChain)) {
        routeKeys.push(fallbackChain);
      }
    }
  }

  const chainLabels = new Map<string, { label: string; shortLabel: string }>(
    routeRows.map((row) => [
      row.key,
      { label: row.label, shortLabel: row.shortLabel },
    ]),
  );
  for (const key of routeKeys) {
    if (!chainLabels.has(key)) {
      chainLabels.set(key, {
        label: titleCase(key),
        shortLabel: titleCase(key),
      });
    }
  }

  const matrixRows = buildMatrixRows(walletByTokenByChain, routeKeys);
  const matrixTotals = routeKeys.map((chainKey) => {
    const valueUsd = matrixRows.reduce((sum, row) => {
      return (
        sum +
        (row.cells.find((cell) => cell.chainKey === chainKey)?.valueUsd ?? 0)
      );
    }, 0);
    return { chainKey, valueUsd, shareOfWalletPct: 0 };
  });
  const matrixTotalUsd = matrixTotals.reduce(
    (sum, cell) => sum + cell.valueUsd,
    0,
  );

  for (const row of matrixRows) {
    row.weightPct = percentage(row.totalUsd, matrixTotalUsd);
    for (const cell of row.cells) {
      cell.shareOfWalletPct = percentage(cell.valueUsd, matrixTotalUsd);
    }
  }
  for (const cell of matrixTotals) {
    cell.shareOfWalletPct = percentage(cell.valueUsd, matrixTotalUsd);
  }

  const hasLiveInvestedBalances = liveInvestedUsd > 0.005;
  const investedUsd = useLiveWalletTokenBalances
    ? liveInvestedUsd
    : hasLiveInvestedBalances
      ? liveInvestedUsd
      : positionMetrics.investedUsd;
  const cashSplit = deriveCashSplit({
    unifiedUsdc,
    unifiedEurc,
    targetAllocations,
    investedUsd,
    snapshot,
  });
  const confirmedWalletValueUsd = walletBalanceUnavailable
    ? 0
    : cashMatrixTotalUsd;
  const walletValueUsd = walletBalanceLoading
    ? cashMatrixTotalUsd
    : confirmedWalletValueUsd;
  const netWorthUsd = investedUsd + walletValueUsd;
  const targetBySymbol = new Map(
    targetAllocations.map((row) => [row.symbol, row.targetWeight]),
  );
  const investedBySymbol = new Map<string, number>();
  if (!useLiveWalletTokenBalances && !hasLiveInvestedBalances) {
    for (const position of positionMetrics.positions ?? []) {
      investedBySymbol.set(
        position.symbol,
        (investedBySymbol.get(position.symbol) ?? 0) + position.valueUsd,
      );
    }
  }
  const symbols = new Set<string>([
    ...matrixRows.map((row) => row.symbol),
    ...investedBySymbol.keys(),
    ...targetBySymbol.keys(),
  ]);
  const tokens = [...symbols]
    .map((symbol) => {
      const walletUsd =
        matrixRows.find((row) => row.symbol === symbol)?.totalUsd ?? 0;
      const investedForSymbol = investedBySymbol.get(symbol) ?? 0;
      const totalUsd = walletUsd + investedForSymbol;
      return {
        symbol,
        walletUsd,
        investedUsd: investedForSymbol,
        totalUsd,
        targetWeight: targetBySymbol.get(symbol as AssetSymbol) ?? 0,
        weightPct: percentage(totalUsd, netWorthUsd),
      };
    })
    .filter((token) => token.totalUsd > 0.005 || token.targetWeight > 0)
    .sort((a, b) => {
      if (b.totalUsd !== a.totalUsd) return b.totalUsd - a.totalUsd;
      if (b.targetWeight !== a.targetWeight)
        return b.targetWeight - a.targetWeight;
      return a.symbol.localeCompare(b.symbol);
    });

  const chains = routeKeys
    .map((key) => {
      const valueUsd =
        matrixTotals.find((cell) => cell.chainKey === key)?.valueUsd ?? 0;
      const labels = chainLabels.get(key)!;
      return {
        key,
        label: labels.label,
        shortLabel: labels.shortLabel,
        totalUsd: valueUsd,
        weightPct: percentage(valueUsd, matrixTotalUsd),
      };
    })
    .sort((a, b) => {
      if (b.totalUsd !== a.totalUsd) return b.totalUsd - a.totalUsd;
      return a.shortLabel.localeCompare(b.shortLabel);
    });

  const hasAgentTarget = targetAllocations.length > 0;
  const hasInvestedPositions = investedUsd > 0.5;
  const hasIdleCash = gatewayBalanceStatus === "ready" && walletValueUsd > 0.5;
  const maxTargetDriftPct = hasLiveInvestedBalances
    ? reviewableDriftPct(tokens)
    : positionMetrics.maxDriftPct;
  const hasReviewableDrift = maxTargetDriftPct >= 5;

  return {
    netWorthUsd,
    investedUsd,
    walletValueUsd,
    reserveUsd: cashSplit.reserveUsd,
    reservePct: cashSplit.usdcTargetWeight,
    deployableUsd: cashSplit.deployableUsd,
    unifiedUsdc,
    unifiedEurc,
    eurcUsd: cashSplit.eurcUsd,
    hasIdleCash,
    hasAgentTarget,
    hasInvestedPositions,
    hasReviewableDrift,
    maxTargetDriftPct,
    walletBalanceLoading,
    walletBalanceUnavailable,
    gatewayBalanceError,
    gatewayBalanceUpdatedAt,
    perChainUsdc,
    status: deriveStatus({
      wallet,
      hasIdleCash,
      hasAgentTarget,
      hasInvestedPositions,
      hasReviewableDrift,
      deployableUsd: cashSplit.deployableUsd,
      walletBalanceLoading,
      walletBalanceUnavailable,
    }),
    tokens,
    chains,
    matrixRows,
    matrixTotals,
    matrixTotalUsd,
    tokenCount: matrixRows.length,
    chainCount: chains.length,
    addressCount: wallet?.networks?.length ?? 0,
  };
}

function buildMatrixRows(
  walletByTokenByChain: Map<string, Map<string, number>>,
  routeKeys: string[],
): DashboardMatrixRow[] {
  return [...walletByTokenByChain.entries()]
    .map(([symbol, byChain]) => {
      const cells = routeKeys.map((chainKey) => ({
        chainKey,
        valueUsd: byChain.get(chainKey) ?? 0,
        shareOfWalletPct: 0,
      }));
      return {
        symbol,
        cells,
        totalUsd: cells.reduce((sum, cell) => sum + cell.valueUsd, 0),
        weightPct: 0,
      };
    })
    .filter((row) => row.totalUsd > 0.005)
    .sort((a, b) => {
      if (b.totalUsd !== a.totalUsd) return b.totalUsd - a.totalUsd;
      return a.symbol.localeCompare(b.symbol);
    });
}

function deriveStatus({
  wallet,
  hasIdleCash,
  hasAgentTarget,
  hasInvestedPositions,
  hasReviewableDrift,
  deployableUsd,
  walletBalanceLoading,
  walletBalanceUnavailable,
}: {
  wallet: WalletInfo | null | undefined;
  hasIdleCash: boolean;
  hasAgentTarget: boolean;
  hasInvestedPositions: boolean;
  hasReviewableDrift: boolean;
  deployableUsd: number;
  walletBalanceLoading: boolean;
  walletBalanceUnavailable: boolean;
}): DashboardStatus {
  if (walletBalanceUnavailable) {
    return {
      label: "Balance check failed",
      detail: "Wallet actions are paused until Circle balance data refreshes.",
      tone: "warn",
    };
  }
  if (walletBalanceLoading) {
    return {
      label: "Syncing wallet",
      detail: "Waiting for the current Circle Gateway balance.",
      tone: "agent",
    };
  }
  if (!wallet) {
    return {
      label: "Wallet missing",
      detail: "Connect or restore a wallet before the agent can act.",
      tone: "warn",
    };
  }
  if (!hasIdleCash && !hasInvestedPositions) {
    return {
      label: "Waiting for funds",
      detail: "Add test USDC to start the first portfolio review.",
      tone: "muted",
    };
  }
  if (!hasAgentTarget && hasIdleCash) {
    return {
      label: "Target needed",
      detail: "Cash is available; the agent needs an approved target mix.",
      tone: "agent",
    };
  }
  if (hasInvestedPositions && hasReviewableDrift && hasAgentTarget) {
    return {
      label: "Drift needs review",
      detail: "Positions moved far enough from target to review a plan.",
      tone: "warn",
    };
  }
  if (deployableUsd > 5 && hasAgentTarget) {
    return {
      label: hasInvestedPositions
        ? "Ready to rebalance"
        : "Awaiting first approval",
      detail: "Deployable wallet cash is ready for a review plan.",
      tone: "warn",
    };
  }
  if (hasInvestedPositions) {
    return {
      label: "Monitoring",
      detail: "No cash is queued; Aegis is watching drift and market changes.",
      tone: "pnl",
    };
  }
  return {
    label: "Reserve held",
    detail: "Wallet cash is inside the current reserve target.",
    tone: "pnl",
  };
}

function addWalletTokenValue(
  walletByTokenByChain: Map<string, Map<string, number>>,
  symbol: string,
  chainKey: string,
  valueUsd: number,
) {
  if (!Number.isFinite(valueUsd) || valueUsd <= 0) return;
  const byChain = walletByTokenByChain.get(symbol) ?? new Map<string, number>();
  byChain.set(chainKey, (byChain.get(chainKey) ?? 0) + valueUsd);
  walletByTokenByChain.set(symbol, byChain);
}

function priceUsdForSymbol(
  symbol: string,
  snapshot: MarketSnapshot | null | undefined,
  eurcUsd: number,
) {
  if (symbol === USDC || symbol === USYC) return 1;
  if (symbol === EURC) return eurcUsd;
  return (
    snapshot?.assets.find((asset) => asset.symbol === symbol)?.priceUsd ?? 0
  );
}

function isCashSymbol(symbol: string) {
  return symbol === USDC || symbol === EURC || symbol === USYC;
}

function percentage(value: number, total: number) {
  if (total <= 0) return 0;
  return (value / total) * 100;
}

function titleCase(value: string) {
  return value
    .replaceAll("-", " ")
    .replaceAll("_", " ")
    .toLowerCase()
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}
