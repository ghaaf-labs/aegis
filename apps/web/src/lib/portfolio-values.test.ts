import { describe, expect, it } from "vitest";
import { derivePortfolioPositionMetrics } from "./portfolio-values";
import type { MarketSnapshot, Portfolio } from "@/types";

describe("derivePortfolioPositionMetrics", () => {
  it("uses one live invested value across totals, weights, and drift", () => {
    const portfolio = {
      id: "p1",
      userId: "u1",
      name: "Value Clarity",
      totalValueUsd: 1200,
      totalPnlUsd: 0,
      totalPnlPct: 0,
      riskScore: 42,
      goal: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      allocations: [
        {
          assetId: "BTC",
          symbol: "BTC",
          quantity: 0.01,
          targetWeight: 50,
          currentWeight: 70,
          valueUsd: 840,
        },
        {
          assetId: "ETH",
          symbol: "ETH",
          quantity: 0.1,
          targetWeight: 30,
          currentWeight: 25,
          valueUsd: 300,
        },
        {
          assetId: "USYC",
          symbol: "USYC",
          quantity: 60,
          targetWeight: 20,
          currentWeight: 5,
          valueUsd: 60,
        },
      ],
    } satisfies Portfolio;

    const snapshot = {
      id: "s1",
      capturedAt: new Date().toISOString(),
      fearGreedIndex: 50,
      totalMarketCapUsd: 0,
      btcDominance: 0,
      assets: [
        {
          symbol: "BTC",
          priceUsd: 77_000,
          change24h: 0,
          change7d: 0,
          marketCap: 0,
          volume24h: 0,
          updatedAt: new Date().toISOString(),
        },
        {
          symbol: "ETH",
          priceUsd: 2_100,
          change24h: 0,
          change7d: 0,
          marketCap: 0,
          volume24h: 0,
          updatedAt: new Date().toISOString(),
        },
      ],
    } satisfies MarketSnapshot;

    const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);

    expect(metrics.investedUsd).toBe(1040);
    expect(metrics.positions.map((position) => position.currentWeight)).toEqual(
      [74.03846153846155, 20.192307692307693, 5.769230769230769],
    );
    expect(metrics.maxDriftPct).toBeCloseTo(24.038, 3);
    expect(metrics.usingLivePrices).toBe(true);
  });

  it("does not turn target setup quantities into invested holdings", () => {
    const portfolio = {
      id: "p1",
      userId: "u1",
      name: "Fresh Target",
      totalValueUsd: 0,
      totalPnlUsd: 0,
      totalPnlPct: 0,
      riskScore: 50,
      goal: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      allocations: [
        {
          assetId: "USDC",
          symbol: "USDC",
          quantity: 12_000,
          targetWeight: 45,
          currentWeight: 0,
          valueUsd: 0,
        },
        {
          assetId: "USYC",
          symbol: "USYC",
          quantity: 7_000,
          targetWeight: 35,
          currentWeight: 0,
          valueUsd: 0,
        },
      ],
    } satisfies Portfolio;

    const metrics = derivePortfolioPositionMetrics(portfolio, null);

    expect(metrics.investedUsd).toBe(0);
    expect(metrics.positions.map((position) => position.valueUsd)).toEqual([
      0, 0,
    ]);
    expect(metrics.maxDriftPct).toBe(45);
    expect(metrics.usingLivePrices).toBe(false);
  });

  it("keeps live marking for confirmed nonzero position values", () => {
    const portfolio = {
      id: "p1",
      userId: "u1",
      name: "Confirmed",
      totalValueUsd: 100,
      totalPnlUsd: 0,
      totalPnlPct: 0,
      riskScore: 50,
      goal: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      allocations: [
        {
          assetId: "USDC",
          symbol: "USDC",
          quantity: 90,
          targetWeight: 100,
          currentWeight: 100,
          valueUsd: 90,
        },
      ],
    } satisfies Portfolio;

    const metrics = derivePortfolioPositionMetrics(portfolio, null);

    expect(metrics.investedUsd).toBe(90);
    expect(metrics.usingLivePrices).toBe(true);
  });

  it("keeps stored economic value when testnet fills imply an impossible mark", () => {
    const portfolio = {
      id: "p1",
      userId: "u1",
      name: "Testnet Fill",
      totalValueUsd: 160,
      totalPnlUsd: 0,
      totalPnlPct: 0,
      riskScore: 50,
      goal: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      allocations: [
        {
          assetId: "ETH",
          symbol: "ETH",
          quantity: 0.46,
          targetWeight: 100,
          currentWeight: 100,
          valueUsd: 160,
        },
      ],
    } satisfies Portfolio;
    const snapshot = {
      id: "s1",
      capturedAt: new Date().toISOString(),
      fearGreedIndex: 50,
      totalMarketCapUsd: 0,
      btcDominance: 0,
      assets: [
        {
          symbol: "ETH",
          priceUsd: 2_100,
          change24h: 0,
          change7d: 0,
          marketCap: 0,
          volume24h: 0,
          updatedAt: new Date().toISOString(),
        },
      ],
    } satisfies MarketSnapshot;

    const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);

    expect(metrics.investedUsd).toBe(160);
    expect(metrics.usingLivePrices).toBe(false);
  });
});
