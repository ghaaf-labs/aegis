import { describe, expect, it } from "vitest";
import type { MarketSnapshot, Portfolio, WalletInfo } from "@/types";
import { deriveDashboardBalanceModel } from "./dashboard-balance-model";

describe("deriveDashboardBalanceModel", () => {
  it("builds one reconciled wallet model for the control tower and matrix", () => {
    const model = deriveDashboardBalanceModel({
      portfolio: portfolio(),
      snapshot: snapshot(),
      wallet: wallet(),
      unifiedUsdc: 592,
      unifiedEurc: 60,
      perChainUsdc: {
        arc: 372,
        base: 40,
        "eth-sepolia": 100,
        "arb-sepolia": 40,
        "avax-fuji": 40,
      },
      perChainEurc: {
        arc: 20,
        base: 20,
        "eth-sepolia": 20,
      },
      gatewayBalanceStatus: "ready",
      gatewayBalanceError: null,
      gatewayBalanceUpdatedAt: 1_790_000_000_000,
    });

    expect(model.netWorthUsd).toBeCloseTo(661.6, 1);
    expect(model.reserveUsd).toBeCloseTo(177.6, 2);
    expect(model.deployableUsd).toBeCloseTo(414.4, 2);
    expect(model.tokenCount).toBe(2);
    expect(model.chainCount).toBe(5);
    expect(model.status.label).toBe("Awaiting first approval");
    expect(model.matrixRows.map((row) => row.symbol)).toEqual(["USDC", "EURC"]);
    expect(
      model.chains.map((chain) => [chain.shortLabel, chain.totalUsd]),
    ).toEqual([
      ["Arc", 395.2],
      ["Ethereum", 123.2],
      ["Base", 63.2],
      ["Arbitrum", 40],
      ["Avalanche", 40],
    ]);
  });

  it("accepts additional token balances without hard-coding USDC and EURC", () => {
    const model = deriveDashboardBalanceModel({
      portfolio: portfolio(),
      snapshot: snapshot(),
      wallet: wallet(),
      unifiedUsdc: 100,
      unifiedEurc: 0,
      perChainUsdc: { arc: 100 },
      perChainEurc: {},
      gatewayBalanceStatus: "ready",
      gatewayBalanceError: null,
      gatewayBalanceUpdatedAt: null,
      extraTokenBalancesByChain: {
        base: { SOL: 2 },
        optimism: { LINK: 3 },
      },
    });

    expect(model.matrixRows.map((row) => row.symbol)).toEqual([
      "SOL",
      "USDC",
      "LINK",
    ]);
    expect(model.chains.map((chain) => chain.key)).toContain("optimism");
    expect(model.tokenCount).toBe(3);
    expect(model.matrixTotalUsd).toBeCloseTo(292.01, 2);
  });

  it("values live wallet token balances from wallet quantity and market price", () => {
    const economicPortfolio = {
      ...portfolio(),
      totalValueUsd: 160,
      allocations: [
        {
          assetId: "ETH",
          symbol: "ETH",
          quantity: 0.4,
          targetWeight: 30,
          currentWeight: 100,
          valueUsd: 160,
        },
      ],
    };

    const model = deriveDashboardBalanceModel({
      portfolio: economicPortfolio,
      snapshot: snapshot(),
      wallet: wallet(),
      unifiedUsdc: 100,
      unifiedEurc: 0,
      perChainUsdc: { arc: 100 },
      perChainEurc: {},
      gatewayBalanceStatus: "ready",
      gatewayBalanceError: null,
      gatewayBalanceUpdatedAt: null,
      extraTokenBalancesByChain: {
        base: { ETH: 0.46 },
      },
    });

    expect(model.investedUsd).toBeCloseTo(966, 2);
    expect(model.netWorthUsd).toBeCloseTo(1066, 2);
    expect(model.matrixRows.map((row) => row.symbol)).toEqual(["ETH", "USDC"]);
    expect(
      model.tokens.find((token) => token.symbol === "ETH")?.totalUsd,
    ).toBeCloseTo(966, 2);
  });

  it("does not invent live token holdings from the stale portfolio ledger", () => {
    const economicPortfolio = {
      ...portfolio(),
      totalValueUsd: 160,
      allocations: [
        {
          assetId: "ETH",
          symbol: "ETH",
          quantity: 0.4,
          targetWeight: 30,
          currentWeight: 100,
          valueUsd: 160,
        },
      ],
    };

    const model = deriveDashboardBalanceModel({
      portfolio: economicPortfolio,
      snapshot: snapshot(),
      wallet: wallet(),
      unifiedUsdc: 100,
      unifiedEurc: 0,
      perChainUsdc: { arc: 100 },
      perChainEurc: {},
      gatewayBalanceStatus: "ready",
      gatewayBalanceError: null,
      gatewayBalanceUpdatedAt: null,
      extraTokenBalancesByChain: {},
    });

    expect(model.investedUsd).toBe(0);
    expect(model.matrixRows.map((row) => row.symbol)).toEqual(["USDC"]);
    expect(model.tokens.find((token) => token.symbol === "ETH")?.totalUsd).toBe(
      0,
    );
  });

  it("surfaces sell-only target drift even when deployable cash is zero", () => {
    const model = deriveDashboardBalanceModel({
      portfolio: {
        ...portfolio(),
        allocations: [
          {
            assetId: "ETH",
            symbol: "ETH",
            quantity: 0.5,
            targetWeight: 20,
            currentWeight: 100,
            valueUsd: 1_050,
          },
        ],
      },
      snapshot: snapshot(),
      wallet: wallet(),
      unifiedUsdc: 0,
      unifiedEurc: 0,
      perChainUsdc: {},
      perChainEurc: {},
      gatewayBalanceStatus: "ready",
      gatewayBalanceError: null,
      gatewayBalanceUpdatedAt: null,
      extraTokenBalancesByChain: {
        base: { ETH: 0.5 },
      },
    });

    expect(model.deployableUsd).toBe(0);
    expect(model.hasReviewableDrift).toBe(true);
    expect(model.status.label).toBe("Drift needs review");
  });
});

function portfolio(): Portfolio {
  const now = new Date().toISOString();
  return {
    id: "portfolio-1",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [],
    riskScore: 0,
    goal: {
      objective: "grow",
      horizon: "1y",
      riskTolerance: "aggressive",
      targetAllocation: { USDC: 30, cbBTC: 40, ETH: 30 },
      includeUsyc: false,
      includeEurc: true,
      createdAt: now,
    },
    createdAt: now,
    updatedAt: now,
  };
}

function wallet(): WalletInfo {
  return {
    walletId: "circle-wallet-1",
    arcAddress: "0x1234567890abcdef1234567890abcdef12345678",
    baseAddress: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
    networks: [
      network("ARC-TESTNET"),
      network("BASE-SEPOLIA"),
      network("ETH-SEPOLIA"),
      network("ARB-SEPOLIA"),
      network("AVAX-FUJI"),
    ],
    createdAt: new Date().toISOString(),
  };
}

function network(blockchain: string) {
  return {
    blockchain,
    walletId: `${blockchain}-wallet`,
    address: "0x1234567890abcdef1234567890abcdef12345678",
    accountType: "EOA",
    state: "LIVE",
  };
}

function snapshot(): MarketSnapshot {
  const now = new Date().toISOString();
  return {
    id: "market-1",
    fearGreedIndex: 25,
    totalMarketCapUsd: 2_600_000_000_000,
    btcDominance: 58,
    capturedAt: now,
    assets: [
      asset("EURC", 1.16),
      asset("SOL", 80),
      asset("LINK", 10.67),
      asset("ETH", 2_100),
      asset("BTC", 77_000),
    ],
  };
}

function asset(
  symbol: string,
  priceUsd: number,
): MarketSnapshot["assets"][number] {
  return {
    symbol: symbol as MarketSnapshot["assets"][number]["symbol"],
    priceUsd,
    change24h: 0,
    change7d: 0,
    marketCap: 0,
    volume24h: 0,
    updatedAt: new Date().toISOString(),
  };
}
