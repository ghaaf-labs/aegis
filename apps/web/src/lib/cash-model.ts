import type { AssetSymbol, MarketSnapshot } from "@/types";

/// Mid-market EURC→USD fallback when the live snapshot has no EURC mark. Matches
/// the approximation used across the cash surfaces before this helper unified them.
const EURC_USD_FALLBACK = 1.085;

interface TargetAllocationLike {
  symbol: AssetSymbol | string;
  targetWeight: number;
}

export interface CashSplitInput {
  /** Unified USDC across every chain wallet (Circle Gateway). */
  unifiedUsdc: number;
  /** Unified EURC across every chain wallet. */
  unifiedEurc: number;
  /** The agent's target allocation rows (from `targetAllocationsForPortfolio`). */
  targetAllocations: TargetAllocationLike[];
  /** Confirmed invested position value in USD (excludes wallet cash). */
  investedUsd: number;
  snapshot: MarketSnapshot | null | undefined;
}

export interface CashSplit {
  /** EURC→USD rate used (live snapshot mark or the stable fallback). */
  eurcUsd: number;
  /** Total wallet cash in USD: USDC + EURC valued at `eurcUsd`. */
  totalWalletUsd: number;
  /** The USDC sleeve the agent targets, as a percent 0–100. */
  usdcTargetWeight: number;
  /** True when a target allocation with a USDC reserve sleeve exists. */
  hasUsdcReserveTarget: boolean;
  /** USD the agent intends to hold as the USDC reserve (not deployable). */
  reserveUsd: number;
  /** USDC surplus *above* the reserve — the only cash actually deployable. */
  deployableUsd: number;
}

/** Live EURC mark from the snapshot, or the stable fallback. Internal — exposed
 * to callers only through `totalWalletCashUsd` / `deriveCashSplit`. */
function eurcUsdPrice(snapshot: MarketSnapshot | null | undefined): number {
  return (
    snapshot?.assets.find((asset) => asset.symbol === "EURC")?.priceUsd ??
    EURC_USD_FALLBACK
  );
}

/** Total wallet cash in USD (USDC + EURC@rate). The one definition of
 * "wallet cash" — retires the ad-hoc `deriveIdleCashUsd`. */
export function totalWalletCashUsd(
  unifiedUsdc: number,
  unifiedEurc: number,
  snapshot: MarketSnapshot | null | undefined,
): number {
  return unifiedUsdc + unifiedEurc * eurcUsdPrice(snapshot);
}

/**
 * The single source of truth for how wallet cash splits into the agent's USDC
 * reserve vs. the deployable surplus. USDC is a first-class *allocation* — the
 * agent targets a % of the portfolio held as USDC — so only the USDC *above*
 * that reserve is deployable. When the portfolio is at target, surplus ≈ $0.
 *
 * Used by every cash surface (dashboard, wallets, analytics, agent-studio, the
 * idle-cash + summary cards) so the numbers reconcile everywhere.
 */
export function deriveCashSplit({
  unifiedUsdc,
  unifiedEurc,
  targetAllocations,
  investedUsd,
  snapshot,
}: CashSplitInput): CashSplit {
  const eurcUsd = eurcUsdPrice(snapshot);
  const totalWalletUsd = unifiedUsdc + unifiedEurc * eurcUsd;
  const usdcTargetWeight =
    targetAllocations.find((a) => a.symbol === "USDC")?.targetWeight ?? 0;
  // Plan basis = invested positions + idle USDC. EURC is intentionally excluded:
  // it is an FX *sleeve* the agent targets, not deployable settlement cash (only
  // USDC is bridged/swapped to rebalance). One consequence: a wallet holding only
  // EURC shows $0 deployable until it holds USDC. `totalWalletUsd` above still
  // counts EURC for the "wallet cash" headline; this basis drives the USDC split.
  const totalValue = investedUsd + unifiedUsdc;
  const hasUsdcReserveTarget =
    targetAllocations.length > 0 && usdcTargetWeight > 0;
  const reserveUsd = hasUsdcReserveTarget
    ? (usdcTargetWeight / 100) * totalValue
    : 0;
  // Only the surplus above the intended reserve can move. With no USDC target
  // sleeve, all idle USDC is deployable (nothing is reserved).
  const deployableUsd = hasUsdcReserveTarget
    ? Math.max(0, unifiedUsdc - reserveUsd)
    : unifiedUsdc;

  return {
    eurcUsd,
    totalWalletUsd,
    usdcTargetWeight,
    hasUsdcReserveTarget,
    reserveUsd,
    deployableUsd,
  };
}
