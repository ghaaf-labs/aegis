import type { AssetSymbol, MarketSnapshot, Portfolio } from "@/types";

const STABLE_PRICE_USD: Partial<Record<AssetSymbol, number>> = {
  USDC: 1,
  USYC: 1,
};

interface PositionMetric {
  symbol: AssetSymbol;
  quantity: number;
  targetWeight: number;
  valueUsd: number;
  currentWeight: number;
  driftPct: number;
}

interface PortfolioPositionMetrics {
  investedUsd: number;
  positions: PositionMetric[];
  usingLivePrices: boolean;
  maxDriftPct: number;
}

export function deriveIdleCashUsd(
  unifiedUsdc: number,
  unifiedEurc: number,
  snapshot: MarketSnapshot | null | undefined,
): number {
  return unifiedUsdc + unifiedEurc * eurcUsdPrice(snapshot);
}

function eurcUsdPrice(snapshot: MarketSnapshot | null | undefined): number {
  return priceUsdForSymbol(snapshot, "EURC") ?? 1.085;
}

export function derivePortfolioPositionMetrics(
  portfolio: Portfolio | null | undefined,
  snapshot: MarketSnapshot | null | undefined,
): PortfolioPositionMetrics {
  if (!portfolio) {
    return {
      investedUsd: 0,
      positions: [],
      usingLivePrices: false,
      maxDriftPct: 0,
    };
  }

  const positions = (portfolio.allocations ?? []).map((allocation) => {
    const price = priceUsdForSymbol(snapshot, allocation.symbol);
    const liveValue =
      price !== null && allocation.quantity > 0 && allocation.valueUsd > 0
        ? price * allocation.quantity
        : null;
    return {
      symbol: allocation.symbol,
      quantity: allocation.quantity,
      targetWeight: allocation.targetWeight,
      valueUsd:
        liveValue !== null && liveValue > 0 ? liveValue : allocation.valueUsd,
      currentWeight: 0,
      driftPct: 0,
      usedLivePrice: liveValue !== null && liveValue > 0,
    };
  });

  const derivedUsd = positions.reduce((sum, position) => {
    return sum + position.valueUsd;
  }, 0);
  const investedUsd =
    derivedUsd > 0.5 ? derivedUsd : (portfolio.totalValueUsd ?? 0);
  const withWeights = positions.map((position) => {
    const currentWeight =
      investedUsd > 0.5 ? (position.valueUsd / investedUsd) * 100 : 0;
    return {
      symbol: position.symbol,
      quantity: position.quantity,
      targetWeight: position.targetWeight,
      valueUsd: position.valueUsd,
      currentWeight,
      driftPct: currentWeight - position.targetWeight,
    };
  });

  return {
    investedUsd,
    positions: withWeights,
    usingLivePrices: positions.some((position) => position.usedLivePrice),
    maxDriftPct: withWeights.reduce((max, position) => {
      return Math.max(max, Math.abs(position.driftPct));
    }, 0),
  };
}

function priceUsdForSymbol(
  snapshot: MarketSnapshot | null | undefined,
  symbol: AssetSymbol,
): number | null {
  const stable = STABLE_PRICE_USD[symbol];
  if (stable !== undefined) return stable;
  return (
    snapshot?.assets.find((asset) => asset.symbol === symbol)?.priceUsd ?? null
  );
}
