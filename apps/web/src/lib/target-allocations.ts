import type { Allocation, AssetSymbol, Portfolio } from "@/types";
import { COMING_SOON_TOKEN_IDS } from "@/lib/route-capabilities";

type TargetAllocationRow = Pick<
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
  if (portfolio.allocations.length > 0) {
    return sweepComingSoonTargets(portfolio.allocations);
  }

  const targetRows = Object.entries(portfolio.goal?.targetAllocation ?? {})
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

  return sweepComingSoonTargets(targetRows);
}

const CASH_SYMBOL = "USDC";
const COMING_SOON_SYMBOLS = new Set<string>(COMING_SOON_TOKEN_IDS);

function sweepComingSoonTargets(
  rows: TargetAllocationRow[],
): TargetAllocationRow[] {
  let sweptTargetWeight = 0;
  const kept: TargetAllocationRow[] = [];

  for (const row of rows) {
    if (isComingSoonTargetOnly(row)) {
      sweptTargetWeight += row.targetWeight;
      continue;
    }
    kept.push({ ...row });
  }

  if (sweptTargetWeight <= 0) return kept;

  const cashRow = kept.find((row) => row.symbol === CASH_SYMBOL);
  if (cashRow) {
    cashRow.targetWeight = roundWeight(
      cashRow.targetWeight + sweptTargetWeight,
    );
    return kept;
  }

  return [
    ...kept,
    {
      assetId: "target-USDC",
      symbol: CASH_SYMBOL,
      quantity: 0,
      targetWeight: roundWeight(sweptTargetWeight),
      currentWeight: 0,
      valueUsd: 0,
    },
  ];
}

function isComingSoonTargetOnly(row: TargetAllocationRow): boolean {
  return (
    COMING_SOON_SYMBOLS.has(row.symbol) &&
    row.quantity <= 0 &&
    row.currentWeight <= 0 &&
    row.valueUsd <= 0
  );
}

function roundWeight(weight: number): number {
  return Math.round(weight * 100) / 100;
}
