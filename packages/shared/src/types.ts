// ── Core domain types shared between frontend and backend ──────────────────

export type AssetSymbol = string;
export type UserId = string;
export type PortfolioId = string;

export interface Asset {
  id: string;
  symbol: AssetSymbol;
  name: string;
  logoUrl?: string;
  coingeckoId: string;
}

export interface AssetPrice {
  symbol: AssetSymbol;
  priceUsd: number;
  change24h: number;
  change7d: number;
  marketCap: number;
  volume24h: number;
  updatedAt: string;
}

export interface Allocation {
  assetId: string;
  symbol: AssetSymbol;
  quantity: number;
  targetWeight: number;
  currentWeight: number;
  valueUsd: number;
}

export interface Portfolio {
  id: PortfolioId;
  userId: UserId;
  name: string;
  totalValueUsd: number;
  totalPnlUsd: number;
  totalPnlPct: number;
  allocations: Allocation[];
  riskScore: number;
  createdAt: string;
  updatedAt: string;
}

export type RiskTolerance = "conservative" | "moderate" | "aggressive";

export interface UserProfile {
  id: UserId;
  email: string;
  riskTolerance: RiskTolerance;
  investmentHorizonMonths: number;
  createdAt: string;
}

export interface AgentDecision {
  id: string;
  portfolioId: PortfolioId;
  reasoning: string;
  recommendation: RebalanceRecommendation;
  confidence: number;
  triggeredBy: AgentTrigger;
  createdAt: string;
}

export type AgentTrigger =
  | "market_movement"
  | "drift_threshold"
  | "risk_breach"
  | "scheduled"
  | "user_request";

export interface RebalanceRecommendation {
  summary: string;
  trades: ProposedTrade[];
  expectedImpact: {
    riskDelta: number;
    diversificationScore: number;
  };
}

export interface ProposedTrade {
  assetId: string;
  symbol: AssetSymbol;
  action: "buy" | "sell";
  quantity: number;
  valueUsd: number;
  reason: string;
}

export interface MarketSnapshot {
  id: string;
  assets: AssetPrice[];
  fearGreedIndex: number;
  totalMarketCapUsd: number;
  btcDominance: number;
  capturedAt: string;
}

// ── API response envelope ──────────────────────────────────────────────────

export interface ApiResponse<T> {
  data: T;
  meta?: {
    page?: number;
    pageSize?: number;
    total?: number;
  };
}

export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

// ── WebSocket message types ────────────────────────────────────────────────

export type WsMessageType =
  | "price_update"
  | "agent_decision"
  | "rebalance_proposed"
  | "portfolio_updated"
  | "ping"
  | "pong";

export interface WsMessage<T = unknown> {
  type: WsMessageType;
  payload: T;
  timestamp: string;
}
