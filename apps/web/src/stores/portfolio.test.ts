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

  it("does not erase freshly submitted allocations when the list response arrives", () => {
    usePortfolioStore.getState().addPortfolio(
      portfolio("p1", [
        {
          assetId: "BTC",
          symbol: "BTC",
          quantity: 0,
          targetWeight: 50,
          currentWeight: 0,
          valueUsd: 0,
        },
      ]),
    );

    usePortfolioStore.getState().setPortfolios([portfolio("p1")]);

    const [stored] = usePortfolioStore.getState().portfolios;
    expect(stored).toBeDefined();
    if (!stored) return;
    expect(stored.allocations).toHaveLength(1);
    expect(stored.allocations[0]?.symbol).toBe("BTC");
  });

  it("replaces the previous target when builder saves again", () => {
    usePortfolioStore.getState().addPortfolio(portfolio("p1"));
    usePortfolioStore.getState().addPortfolio(portfolio("p2"));

    const state = usePortfolioStore.getState();
    expect(state.portfolios).toHaveLength(1);
    expect(state.portfolios[0]?.id).toBe("p2");
    expect(state.activePortfolioId).toBe("p2");
  });

  it("uses product language for unknown wallet cash", () => {
    usePortfolioStore.getState().setGatewayBalanceStatus("error");

    expect(usePortfolioStore.getState().gatewayBalanceError).toBe(
      "Wallet balance unavailable",
    );
  });

  it("tracks active dashboard hydration statuses by portfolio", () => {
    const store = usePortfolioStore.getState();

    store.setActivePortfolioDetailStatus("p1", "loading");
    store.setDecisionsStatus("p1", "ready");

    const state = usePortfolioStore.getState();
    expect(state.activePortfolioDetailId).toBe("p1");
    expect(state.activePortfolioDetailStatus).toBe("loading");
    expect(state.decisionsPortfolioId).toBe("p1");
    expect(state.decisionsStatus).toBe("ready");
  });
});

function portfolio(
  id: string,
  allocations: Portfolio["allocations"] = [],
): Portfolio {
  const now = new Date().toISOString();
  return {
    id,
    userId: "user-1",
    name: "Main portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations,
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
}
