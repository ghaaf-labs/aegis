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

export interface RoutePreferences {
  /** Circle chain codes with wallet routes the agent may use for account planning and balance tracking. */
  networks: string[];
  /** Circle chain codes still waiting on wallet sync. */
  networkWatchlist?: string[];
  /** Asset symbols the agent may consider for target plans; execution still gates per adapter. */
  tokens: string[];
  /** Assets the user wants the agent to monitor without using in plans. */
  watchlist: string[];
  updatedAt?: string;
}

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
  /** Agent execution scope chosen from the wallet page. */
  routePreferences?: RoutePreferences;
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

/** Helper: SSE event payloads scoped to a specific user. The API filters
 * these server-side; clients with a different JWT subject never see them. */
export interface UserScopedSseEvent {
  userId: UserId;
}

/** Result of a Circle Wallet create. JWT is set in an httpOnly cookie. */
export interface WalletInfo {
  walletId: string;
  arcAddress: string;
  baseAddress: string;
  networks?: WalletNetwork[];
  createdAt: string;
}

export interface WalletNetwork {
  blockchain: string;
  walletId: string;
  address: string;
  accountType: string;
  state: string;
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
  model?: string;
  /** Modern verdict field (preferred). Adds "veto" for constitution-driven
   * short-circuit decisions surfaced by the F-CON-3 flow. */
  verdict?: "approved" | "revised" | "abstained" | "veto";
  /** Legacy boolean field for older decisions. */
  demandsRevision?: boolean;
  notes: string;
  confidence?: number;
  /** Constitution clause IDs cited by this verdict. Present (non-empty) only
   * on veto verdicts; absent or empty for ordinary critic output. */
  clauseIds?: string[];
}

/** One clause from the Aegis Constitution YAML, surfaced on the public
 * /about/constitution model card and (by id) in the approval modal. */
export interface ConstitutionClause {
  id: string;
  summary: string;
  description: string;
  field: string;
  kind: "hard_limit" | "band" | "floor" | "ceiling";
  tierMin?: "free" | "pro" | "business";
}

export interface ConstitutionDocument {
  version: number;
  effectiveAt: string;
  clauses: ConstitutionClause[];
}

export interface AgentDecision {
  id: string;
  portfolioId: PortfolioId;
  /** Set on SSE-delivered decisions so server-side audience filtering can
   * be verified; absent on REST responses where auth has already gated. */
  userId?: UserId;
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
  /** Decision-time portfolio/wallet snapshot. Shape varies by planner. */
  snapshot?: Record<string, unknown>;

  // F-CONF-4 / F-CONF-5: calibrated confidence + critic counterfactual.
  // Only populated when CALIBRATED_CONF_ENABLED=true on the API; the UI
  // gracefully falls back to `confidence` when these are absent.
  /** Strategist's raw self-reported confidence (0..1). */
  rawConfidence?: number;
  /** Confidence after the A8 histogram-bin calibrator is applied. */
  calibratedConfidence?: number;
  /** One-sentence critic counterfactual (e.g. "If regime had stayed RISK_ON,
   *  this rebalance would not fire."). */
  counterfactual?: string;
}

export type AgentTrigger =
  | "market_movement"
  | "drift_threshold"
  | "risk_breach"
  | "scheduled"
  | "user_request"
  | "regime_flip"
  | "abstain"
  | "peg_alert";

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
  | { type: "agent.tool.invoked"; data: AgentToolInvoked }
  | { type: "agent.abstained"; data: AgentAbstained }
  | { type: "rebalance.status"; data: RebalanceStatus }
  | { type: "rebalance.plan.created"; data: RebalancePlan }
  | { type: "rebalance.leg.update"; data: RebalanceLeg }
  | { type: "tax.harvest.proposed"; data: HarvestableLoss }
  | { type: "gateway.balance"; data: GatewayBalance }
  | { type: "wallet.created"; data: WalletInfo }
  | { type: "referral.credited"; data: ReferralCredited }
  | { type: "peg.alert"; data: PegAlert };

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

/**
 * Strategist mid-decision tool invocation. The reasoning feed shows each
 * call as a single breadcrumb row so users see what the agent looked at.
 */
export interface AgentToolInvoked {
  portfolioId: PortfolioId;
  toolName:
    | "fetch_news"
    | "fetch_onchain_metric"
    | "fetch_correlation"
    | string;
  /** Truncated JSON-stringified preview of the tool's result. */
  resultPreview: string;
  latencyMs: number;
  invokedAt: string;
}

/**
 * Confidence-based abstain — the strategist concluded no action was justified.
 * UI surfaces this as a "agent held off" card with the stated reason.
 */
export interface AgentAbstained {
  portfolioId: PortfolioId;
  confidence: number;
  reason: string;
  decidedAt: string;
}

export interface GatewayBalance extends UserScopedSseEvent {
  unifiedUsdc: number;
  /** Sum of EURC across every chain the user holds a wallet on. */
  unifiedEurc: number;
  /** USDC per chain — keys are lowercased chain shorthands ("arc", "base"). */
  perChain: Record<string, number>;
  /** EURC per chain — same key set as `perChain`. */
  perChainEurc: Record<string, number>;
  observedAt: string;
}

// ── Peg-defense (A6) ────────────────────────────────────────────────────────

export type PegAssetSymbol = "USDC" | "EURC" | "USYC";

export type PegActionKind = "alert" | "propose_rebalance" | "auto_execute";

export interface PegRule {
  id: string;
  userId: UserId;
  /** Null = apply across every portfolio the user owns. */
  portfolioId?: PortfolioId | null;
  asset: PegAssetSymbol;
  /** Trigger when observed price drops below this value (e.g. 0.995). */
  thresholdPrice: number;
  /** Rolling window the depeg must persist over before firing. */
  windowSeconds: number;
  actionKind: PegActionKind;
  /** Defensive asset to rotate into when `actionKind != 'alert'`. */
  targetAsset?: PegAssetSymbol | null;
  enabled: boolean;
  pausedAt?: string | null;
  lastFiredAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface PegAlert {
  ruleId: string;
  asset: PegAssetSymbol;
  observedPrice: number;
  thresholdPrice: number;
  observedAt: string;
  actionTaken: PegActionKind;
  rebalanceId?: string;
}

/** Mirrors `billing::service::ReferralCreditedPayload`. Audience = referrer. */
export interface ReferralCredited {
  referrerUserId: string;
  newUserId: string;
  rewardUsdc: number;
  /** Present when the Nanopayment settled; null when still pending. */
  txHash: string | null;
}

// ── Cross-chain rebalance execution (Sprint 3) ─────────────────────────────

export type ChainKey = "arc" | "base";

export type LegKind =
  | "local_swap"
  | "cross_chain_burn"
  | "cross_chain_mint"
  | "park_usyc"
  | "redeem_usyc"
  | "fx_stablefx";

export type LegStatus = "pending" | "submitted" | "confirmed" | "failed";

/**
 * Plain-language execution state for a token/route, mirrored from the backend
 * route registry (`RouteState`). The UI must use exactly these labels so users
 * never mistake a non-executable route for a working one:
 * - `ready` — can execute now
 * - `track-only` — price-tracked but not executable (disabled / KYB-gated)
 * - `needs-route` — adapter or rail not connected
 * - `needs-quote` — live route, awaiting a fresh quote
 * - `needs-address` — live adapter, but the token's on-chain address is unset
 */
export type RouteState =
  | "ready"
  | "track-only"
  | "needs-route"
  | "needs-quote"
  | "needs-address";

export type RebalanceLifecycle =
  | "planned"
  | "approved"
  | "executing"
  | "completed"
  | "failed"
  | "cancelled";

export interface RebalanceLeg {
  id: string;
  rebalanceId: string;
  legIndex: number;
  kind: LegKind;
  srcChain?: ChainKey;
  destChain?: ChainKey;
  srcSymbol?: AssetSymbol;
  destSymbol?: AssetSymbol;
  amountUsdc: number;
  minOut?: number;
  status: LegStatus;
  txHash?: string;
  cctpMessageHash?: string;
  failureReason?: string;
  submittedAt?: string;
  confirmedAt?: string;
  createdAt: string;
}

export interface RebalancePlan {
  id: string;
  portfolioId: PortfolioId;
  decisionId: string;
  status: RebalanceLifecycle;
  totalLegs: number;
  completedLegs: number;
  totalGasUsdc: number | null;
  failureReason?: string;
  approvedAt?: string;
  completedAt?: string;
  createdAt: string;
  updatedAt: string;
  legs: RebalanceLeg[];
}

export interface CrossChainRoute {
  srcChain: ChainKey;
  destChain: ChainKey;
  amountUsdc: number;
  hookTarget: string;
  estimatedSeconds: number;
  estimatedFeeUsdc: number;
}

export interface HarvestableLoss {
  portfolioId: PortfolioId;
  allocationId: string;
  symbol: AssetSymbol;
  unrealizedLossUsd: number;
  /** Open lots that are currently sitting at a loss vs current price. */
  lots: HarvestableLot[];
  /** Set by the strategist when it explicitly recommends realizing this. */
  proposedAt?: string;
}

export interface HarvestableLot {
  lotId: string;
  acquiredAt: string;
  quantity: number;
  basisUsd: number;
  currentValueUsd: number;
}

export interface DigestSubscription {
  email: string;
  subscribedAt: string;
  lastSentAt?: string;
}

export interface DiaryEntry {
  decisionId: string;
  portfolioId: PortfolioId;
  walletAddress: string;
  regime: MarketRegime;
  modelSlug: string;
  confidence: number;
  recommendationSummary: string;
  createdAt: string;
  outcome?: DiaryOutcome;
  /** Adversarial critic review of the strategist proposal (model, verdict, notes). */
  criticVerdict?: {
    model: string;
    verdict: "approved" | "revised" | "abstained";
    notes: string;
    confidence?: number;
  };
}

export interface DiaryOutcome {
  /** What the portfolio actually did in the 24 hours after the decision. */
  realizedPctChange: number;
  /** What it *would* have done had the recommendation been executed. */
  counterfactualPctChange: number;
  compressedSummary: string;
  recordedAt: string;
}

export interface CounterfactualReplay {
  decisionId: string;
  realizedPct: number;
  counterfactualPct: number;
  deltaPct: number;
}

// ── Billing v2 (subscriptions, invoices, usage, performance fees) ──────────
//
// Mirror of `apps/api/src/modules/billing/types.rs`. All fields are camelCase
// to match the Rust `#[serde(rename_all = "camelCase")]` convention.

export type Tier = "free" | "pro" | "business";

export type SubscriptionStatus = "trialing" | "active" | "pastDue" | "canceled";

export interface Subscription {
  id: string;
  userId: UserId;
  tier: Tier;
  status: SubscriptionStatus;
  startedAt?: string;
  currentPeriodStart: string;
  currentPeriodEnd: string;
  cancelAt?: string | null;
  /** Set when the cancellation has fully taken effect. */
  canceledAt?: string | null;
  /** Day of month (1-28) used as the monthly billing anchor. */
  billingAnchorDay?: number;
  createdAt: string;
  updatedAt: string;
}

export type InvoiceStatus = "open" | "paid" | "void" | "pastDue" | "past_due";

export interface LineItem {
  description: string;
  quantity?: number;
  unitAmountUsdc?: number;
  amountUsdc: number;
  /** Free-form tag, e.g. "subscription", "aum_stream", "rebalance_fee". */
  kind?: string;
}

export interface Invoice {
  id: string;
  userId: UserId;
  subscriptionId?: string;
  /** A5 — tier this invoice was billed at (display-only). */
  tier?: Tier;
  periodStart: string;
  periodEnd: string;
  lineItems: LineItem[];
  subtotalUsdc: number;
  totalUsdc: number;
  status: InvoiceStatus;
  paidAt?: string | null;
  paidTxHash?: string | null;
  createdAt: string;
}

export interface PricingTier {
  code: Tier;
  /** Alias for `code` used by the pricing-UI (A5). */
  tier?: Tier;
  /** Human-readable display name (e.g. "Free" / "Pro" / "Business"). */
  name?: string;
  monthlyUsd: number;
  /** Null on Pro / Business — unlimited AUM. */
  aumCapUsd: number | null;
  /** Null on Business — unlimited portfolios. */
  portfoliosCap: number | null;
  /** A5 alias for `portfoliosCap`. */
  portfolioCap?: number | null;
  /** Null on Business — unlimited decisions. */
  decisionsCapMonthly: number | null;
  /** A5 alias for `decisionsCapMonthly`. */
  decisionsPerMonth?: number | null;
  perRebalanceBps: number;
  aumAnnualBps: number;
  /** Display-only — free-form model menu, e.g. "Haiku regime + Haiku strategist". */
  models?: string;
  /** Display-only — marketing bullet list rendered under the price. */
  features?: string[];
  /** UI hint: render this tier as the recommended middle column. */
  recommended?: boolean;
}

export interface UsageMeter {
  userId: UserId;
  /** Billing-period anchor date (YYYY-MM-DD). */
  periodStart: string;
  decisionsCount: number;
  aumUsdAvg: number;
  updatedAt: string;
}

export type PerformanceBenchmark = "tbill3m" | "susds";

export interface PerformanceFee {
  id: string;
  userId: UserId;
  decisionId?: string;
  period: "monthly";
  benchmark: PerformanceBenchmark;
  realizedGainUsd: number;
  accruedBps: number;
  accruedUsdc: number;
  settledAt?: string;
  settlementTxHash?: string;
  createdAt: string;
}
