import { describe, expect, it } from "vitest";

import { targetAllocationsForPortfolio } from "@/components/dashboard/target-allocations";
import type { Portfolio } from "@/types";

const BASE_PORTFOLIO: Portfolio = {
  id: "portfolio-1",
  userId: "user-1",
  name: "Test portfolio",
  totalValueUsd: 0,
  totalPnlUsd: 0,
  totalPnlPct: 0,
  allocations: [],
  riskScore: 0,
  goal: {
    objective: "preserve",
    horizon: "1y",
    riskTolerance: "conservative",
    targetAllocation: {},
    includeUsyc: false,
    includeEurc: true,
    createdAt: "2026-05-24T00:00:00Z",
  },
  createdAt: "2026-05-24T00:00:00Z",
  updatedAt: "2026-05-24T00:00:00Z",
};

describe("targetAllocationsForPortfolio", () => {
  it("sweeps coming-soon target weights into USDC from goal targets", () => {
    const rows = targetAllocationsForPortfolio({
      ...BASE_PORTFOLIO,
      goal: {
        ...BASE_PORTFOLIO.goal!,
        targetAllocation: { EURC: 10, USDC: 70, USYC: 20 },
      },
    });

    expect(rows.map((row) => row.symbol)).toEqual(["EURC", "USDC"]);
    expect(rows.find((row) => row.symbol === "USDC")?.targetWeight).toBe(90);
  });

  it("sweeps stale coming-soon allocation rows before rendering targets", () => {
    const rows = targetAllocationsForPortfolio({
      ...BASE_PORTFOLIO,
      allocations: [
        {
          assetId: "eurc",
          symbol: "EURC",
          quantity: 0,
          targetWeight: 10,
          currentWeight: 0,
          valueUsd: 0,
        },
        {
          assetId: "usdc",
          symbol: "USDC",
          quantity: 0,
          targetWeight: 70,
          currentWeight: 0,
          valueUsd: 0,
        },
        {
          assetId: "usyc",
          symbol: "USYC",
          quantity: 0,
          targetWeight: 20,
          currentWeight: 0,
          valueUsd: 0,
        },
      ],
    });

    expect(rows.map((row) => row.symbol)).toEqual(["EURC", "USDC"]);
    expect(rows.find((row) => row.symbol === "USDC")?.targetWeight).toBe(90);
  });
});
