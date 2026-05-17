import type {
  Portfolio,
  AgentDecision,
  MarketSnapshot,
  AssetPrice,
} from "@/types";

const MOCK_PRICES: AssetPrice[] = [
  {
    symbol: "BTC",
    priceUsd: 67420.5,
    change24h: 2.4,
    change7d: 8.1,
    marketCap: 1_324_000_000_000,
    volume24h: 28_400_000_000,
    updatedAt: new Date().toISOString(),
  },
  {
    symbol: "ETH",
    priceUsd: 3521.3,
    change24h: 3.1,
    change7d: 12.4,
    marketCap: 423_000_000_000,
    volume24h: 14_200_000_000,
    updatedAt: new Date().toISOString(),
  },
  {
    symbol: "SOL",
    priceUsd: 184.2,
    change24h: -1.2,
    change7d: 6.8,
    marketCap: 87_000_000_000,
    volume24h: 3_800_000_000,
    updatedAt: new Date().toISOString(),
  },
  {
    symbol: "BNB",
    priceUsd: 432.8,
    change24h: 0.8,
    change7d: 4.2,
    marketCap: 66_000_000_000,
    volume24h: 1_900_000_000,
    updatedAt: new Date().toISOString(),
  },
  {
    symbol: "AVAX",
    priceUsd: 38.4,
    change24h: -2.8,
    change7d: -3.1,
    marketCap: 15_800_000_000,
    volume24h: 620_000_000,
    updatedAt: new Date().toISOString(),
  },
  {
    symbol: "LINK",
    priceUsd: 14.72,
    change24h: 1.9,
    change7d: 9.3,
    marketCap: 8_900_000_000,
    volume24h: 540_000_000,
    updatedAt: new Date().toISOString(),
  },
];

export const MOCK_PORTFOLIO: Portfolio = {
  id: "port_01",
  userId: "user_01",
  name: "Main Portfolio",
  totalValueUsd: 48_240.8,
  totalPnlUsd: 6_840.3,
  totalPnlPct: 16.5,
  riskScore: 42,
  goal: {
    name: "Main Portfolio",
    horizon: "5y",
    riskTolerance: "moderate",
    targetAllocation: { BTC: 40, ETH: 30, SOL: 15, LINK: 10, USYC: 5, EURC: 0 },
    includeUsyc: true,
    includeEurc: true,
    createdAt: "2024-01-15T00:00:00Z",
  },
  createdAt: "2024-01-15T00:00:00Z",
  updatedAt: new Date().toISOString(),
  allocations: [
    {
      assetId: "btc",
      symbol: "BTC",
      quantity: 0.42,
      targetWeight: 40,
      currentWeight: 58.7,
      valueUsd: 28_316.61,
    },
    {
      assetId: "eth",
      symbol: "ETH",
      quantity: 3.2,
      targetWeight: 30,
      currentWeight: 23.4,
      valueUsd: 11_268.16,
    },
    {
      assetId: "sol",
      symbol: "SOL",
      quantity: 28.5,
      targetWeight: 15,
      currentWeight: 10.9,
      valueUsd: 5_249.7,
    },
    {
      assetId: "bnb",
      symbol: "BNB",
      quantity: 5.1,
      targetWeight: 10,
      currentWeight: 4.6,
      valueUsd: 2_207.28,
    },
    {
      assetId: "avax",
      symbol: "AVAX",
      quantity: 28,
      targetWeight: 5,
      currentWeight: 2.4,
      valueUsd: 1_075.2,
    },
  ],
};

export const MOCK_AGENT_DECISIONS: AgentDecision[] = [
  {
    id: "dec_01",
    portfolioId: "port_01",
    reasoning:
      "BTC allocation has drifted +18.7% above target. With BTC dominance at 54% and fear/greed index at 72 (Greed), risk-adjusted models suggest trimming exposure to lock in gains and reduce concentration risk.",
    confidence: 0.87,
    triggeredBy: "drift_threshold",
    createdAt: new Date(Date.now() - 12 * 60 * 1000).toISOString(),
    recommendation: {
      summary: "Rebalance: Trim BTC, increase ETH and SOL exposure",
      expectedImpact: { riskDelta: -8, diversificationScore: 0.74 },
      trades: [
        {
          assetId: "btc",
          symbol: "BTC",
          action: "sell",
          quantity: 0.08,
          valueUsd: 5_393.64,
          reason: "Reduce BTC concentration from 58.7% to ~40% target",
        },
        {
          assetId: "eth",
          symbol: "ETH",
          action: "buy",
          quantity: 0.9,
          valueUsd: 3_169.17,
          reason: "ETH underweight vs target (23.4% vs 30%)",
        },
        {
          assetId: "sol",
          symbol: "SOL",
          action: "buy",
          quantity: 11.5,
          valueUsd: 2_117.3,
          reason: "SOL underweight vs target (10.9% vs 15%)",
        },
      ],
    },
  },
  {
    id: "dec_02",
    portfolioId: "port_01",
    reasoning:
      "SOL showing relative strength (+6.8% 7d) while maintaining lower correlation to BTC recent moves. Moderate increase in position size aligned with risk tolerance profile.",
    confidence: 0.71,
    triggeredBy: "market_movement",
    createdAt: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
    recommendation: {
      summary: "Opportunistic: Add to SOL position on strength",
      expectedImpact: { riskDelta: 3, diversificationScore: 0.68 },
      trades: [
        {
          assetId: "sol",
          symbol: "SOL",
          action: "buy",
          quantity: 8,
          valueUsd: 1_473.6,
          reason: "Relative strength breakout with volume confirmation",
        },
      ],
    },
  },
  {
    id: "dec_03",
    portfolioId: "port_01",
    reasoning:
      "Portfolio risk score within acceptable bounds. No significant drift detected. Market conditions stable. Monitoring continues.",
    confidence: 0.95,
    triggeredBy: "scheduled",
    createdAt: new Date(Date.now() - 6 * 60 * 60 * 1000).toISOString(),
    recommendation: {
      summary: "Hold: Portfolio within target parameters",
      expectedImpact: { riskDelta: 0, diversificationScore: 0.65 },
      trades: [],
    },
  },
];

export const MOCK_MARKET_SNAPSHOT: MarketSnapshot = {
  id: "snap_01",
  assets: MOCK_PRICES,
  fearGreedIndex: 72,
  totalMarketCapUsd: 2_420_000_000_000,
  btcDominance: 54.3,
  capturedAt: new Date().toISOString(),
};
