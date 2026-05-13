export const SUPPORTED_ASSETS = [
  { symbol: "BTC", name: "Bitcoin", coingeckoId: "bitcoin" },
  { symbol: "ETH", name: "Ethereum", coingeckoId: "ethereum" },
  { symbol: "SOL", name: "Solana", coingeckoId: "solana" },
  { symbol: "BNB", name: "BNB", coingeckoId: "binancecoin" },
  { symbol: "AVAX", name: "Avalanche", coingeckoId: "avalanche-2" },
  { symbol: "MATIC", name: "Polygon", coingeckoId: "matic-network" },
  { symbol: "LINK", name: "Chainlink", coingeckoId: "chainlink" },
  { symbol: "UNI", name: "Uniswap", coingeckoId: "uniswap" },
] as const;

export const RISK_TOLERANCE_LABELS = {
  conservative: "Conservative",
  moderate: "Moderate",
  aggressive: "Aggressive",
} as const;

export const RISK_SCORE_THRESHOLDS = {
  low: 30,
  medium: 60,
  high: 100,
} as const;

export const REBALANCE_DRIFT_THRESHOLD = 0.05; // 5% drift triggers rebalance consideration

export const API_ROUTES = {
  health: "/health",
  auth: {
    register: "/auth/register",
    login: "/auth/login",
    refresh: "/auth/refresh",
    me: "/auth/me",
  },
  portfolios: {
    list: "/portfolios",
    create: "/portfolios",
    get: (id: string) => `/portfolios/${id}`,
    update: (id: string) => `/portfolios/${id}`,
    delete: (id: string) => `/portfolios/${id}`,
    rebalance: (id: string) => `/portfolios/${id}/rebalance`,
  },
  market: {
    prices: "/market/prices",
    snapshot: "/market/snapshot",
  },
  agent: {
    decisions: (portfolioId: string) => `/agent/decisions/${portfolioId}`,
    analyze: "/agent/analyze",
  },
} as const;
