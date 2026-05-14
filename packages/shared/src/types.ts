// ── Core domain types shared between frontend and backend ──────────────────

export type AssetSymbol =
  | "BTC"
  | "ETH"
  | "SOL"
  | "BNB"
  | "AVAX"
  | "LINK"
  | "UNI"
  | "MATIC"
  | "USYC"
  | "EURC"
  | (string & {});
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
  /** Set when the user completes the goal wizard. Null for legacy portfolios. */
  goal: PortfolioGoal | null;
  createdAt: string;
  updatedAt: string;
}

export type RiskTolerance = "conservative" | "moderate" | "aggressive";

export type GoalHorizon = "1y" | "3y" | "5y" | "10y" | "20y+";

export interface PortfolioGoal {
  /** Human label, e.g. "Retirement", "Treasury", "Speculative". */
  name: string;
  horizon: GoalHorizon;
  riskTolerance: RiskTolerance;
  /** Target weights per symbol; values sum to 100. Sparse — symbols not
   * listed default to 0%. */
  targetAllocation: Partial<Record<AssetSymbol, number>>;
  /** Optional recurring contribution. */
  monthlyContributionUsd?: number;
  /** Always available; default 0 in `targetAllocation`. */
  includeUsyc: boolean;
  /** Always available; default 0 in `targetAllocation`. */
  includeEurc: boolean;
  createdAt: string;
}

export interface UserProfile {
  id: UserId;
  email: string;
  riskTolerance: RiskTolerance;
  investmentHorizonMonths: number;
  walletId?: string;
  arcAddress?: string;
  baseAddress?: string;
  createdAt: string;
}

/** Result of a Circle Wallet create. JWT is set in an httpOnly cookie. */
export interface WalletInfo {
  walletId: string;
  arcAddress: string;
  baseAddress: string;
  createdAt: string;
}

// ── Agent decisions ────────────────────────────────────────────────────────

export type MarketRegime = "risk_on" | "neutral" | "risk_off";

export type ModelRoute =
  | "regime_classify"
  | "rebalance_reason"
  | "tax_explain"
  | "market_commentary"
  | "critique_agent";

export interface CriticVerdict {
  demandsRevision: boolean;
  notes: string;
  /** Confidence the critic has that the strategist's proposal is sound (0..1). */
  confidence: number;
}

export interface AgentDecision {
  id: string;
  portfolioId: PortfolioId;
  reasoning: string;
  recommendation: RebalanceRecommendation;
  /** Strategist's confidence (0..1). */
  confidence: number;
  triggeredBy: AgentTrigger;
  createdAt: string;

  // Telemetry — present from migration 0002 onwards. Optional for back-compat
  // with rows persisted before the rewrite.
  /** Resolved OpenRouter model slug (e.g. `anthropic/claude-opus-4-7`). */
  modelSlug?: string;
  regime?: MarketRegime;
  promptTokens?: number;
  completionTokens?: number;
  latencyMs?: number;
  criticVerdict?: CriticVerdict;
}

export type AgentTrigger =
  | "market_movement"
  | "drift_threshold"
  | "risk_breach"
  | "scheduled"
  | "user_request"
  | "regime_flip"
  | "abstain";

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

// ── Server-Sent Events ─────────────────────────────────────────────────────
//
// One discriminated union for every event the API streams over `/sse`. The
// backend names each event via the `event:` field; the `data` payload is JSON
// matching the shape below.

export type SseEvent =
  | { type: "price.tick"; data: PriceTick }
  | { type: "regime.flip"; data: RegimeFlip }
  | { type: "agent.decision"; data: AgentDecision }
  | { type: "rebalance.status"; data: RebalanceStatus }
  | { type: "gateway.balance"; data: GatewayBalance }
  | { type: "wallet.created"; data: WalletInfo };

export type SseEventType = SseEvent["type"];

/** Map from event type name to its data payload. */
export type SseEventMap = {
  [K in SseEventType]: Extract<SseEvent, { type: K }>["data"];
};

export interface PriceTick {
  symbol: AssetSymbol;
  priceUsd: number;
  change24h: number;
  source: string;
  fetchedAt: string;
}

export interface RegimeFlip {
  from: MarketRegime | null;
  to: MarketRegime;
  confidence: number;
  signals: RegimeSignals;
  classifiedAt: string;
}

export interface RegimeSignals {
  btcVol30d: number;
  corr90d: number;
  maxDrawdown: number;
}

export interface RebalanceStatus {
  id: string;
  step: string;
  chain?: string;
  txHash?: string;
  status: "pending" | "submitted" | "confirmed" | "failed";
  updatedAt: string;
}

export interface GatewayBalance {
  unifiedUsdc: number;
  perChain: Record<string, number>;
  observedAt: string;
}
