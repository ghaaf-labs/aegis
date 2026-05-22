import { afterEach, describe, expect, it } from "vitest";
import { usePortfolioStore } from "./portfolio";
import type { Portfolio } from "@/types";

describe("portfolio store onboarding state", () => {
  afterEach(() => {
    usePortfolioStore.getState().resetSession();
    usePortfolioStore.getState().setSessionResolved(false);
    window.localStorage.clear();
  });

  it("marks portfolios loaded when onboarding creates the first portfolio", () => {
    expect(usePortfolioStore.getState().portfoliosLoaded).toBe(false);

    usePortfolioStore.getState().addPortfolio(portfolio("p1"));

    const state = usePortfolioStore.getState();
    expect(state.portfoliosLoaded).toBe(true);
    expect(state.activePortfolioId).toBe("p1");
    expect(state.portfolios).toHaveLength(1);
  });

  it("uses product language for unknown wallet cash", () => {
    usePortfolioStore.getState().setGatewayBalanceStatus("error");

    expect(usePortfolioStore.getState().gatewayBalanceError).toBe(
      "Wallet balance unavailable",
    );
  });
});

function portfolio(id: string): Portfolio {
  const now = new Date().toISOString();
  return {
    id,
    userId: "user-1",
    name: "Main portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [],
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
}
