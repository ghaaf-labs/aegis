import type { Allocation, AssetSymbol, Portfolio } from "@/types";

export type TargetAllocationRow = Pick<
  Allocation,
  | "assetId"
  | "symbol"
  | "quantity"
  | "targetWeight"
  | "currentWeight"
  | "valueUsd"
>;

export function targetAllocationsForPortfolio(
  portfolio: Portfolio | null | undefined,
): TargetAllocationRow[] {
  if (!portfolio) return [];
  if (portfolio.allocations.length > 0) return portfolio.allocations;

  return Object.entries(portfolio.goal?.targetAllocation ?? {})
    .filter((entry): entry is [AssetSymbol, number] => {
      const [, weight] = entry;
      return (
        typeof weight === "number" && Number.isFinite(weight) && weight > 0
      );
    })
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([symbol, targetWeight]) => ({
      assetId: `target-${symbol}`,
      symbol,
      quantity: 0,
      targetWeight,
      currentWeight: 0,
      valueUsd: 0,
    }));
}
