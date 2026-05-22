import type {
  AgentDecision,
  ChainKey,
  DiaryEntry,
  HarvestableLoss,
  Invoice,
  MarketSnapshot,
  PegActionKind,
  PegAssetSymbol,
  PegRule,
  Portfolio,
  PortfolioGoal,
  PricingTier,
  Subscription,
  Tier,
  WalletInfo,
} from "@/types";

const BASE_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

const REMEMBERED_EMAIL_KEY = "aegis_email";

function clearRememberedAuthState() {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(REMEMBERED_EMAIL_KEY);
}

interface FetchOptions {
  method?: string;
  body?: unknown;
  authed?: boolean;
}

async function request<T>(path: string, opts: FetchOptions = {}): Promise<T> {
  const body = opts.body !== undefined ? JSON.stringify(opts.body) : undefined;

  const res = await fetch(`${BASE_URL}${path}`, {
    method: opts.method ?? "GET",
    headers: {
      "Content-Type": "application/json",
      ...(opts.method && opts.method !== "GET"
        ? { "X-Aegis-Request": "1" }
        : {}),
    },
    // `credentials: 'include'` ensures the httpOnly auth cookie set by the
    // wallet endpoints rides on every cross-origin request. Backend CORS
    // enables `Access-Control-Allow-Credentials` with a specific origin
    // allow-list (no wildcard).
    credentials: "include",
    body,
  });
  if (!res.ok) {
    const detail = await responseErrorDetail(res);
    throw new Error(`${res.status}: ${detail}`);
  }
  // 204 + zero-length bodies (e.g. DELETE endpoints) have nothing
  // to parse — return undefined cast to T rather than throwing on JSON parse.
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

async function responseErrorDetail(res: Response) {
  let detail = res.statusText;
  try {
    const body = await res.json();
    detail =
      body.error?.message ??
      body.message ??
      body.error?.code ??
      body.code ??
      detail;
  } catch {
    /* ignore */
  }
  return detail;
}

// ── Wallet auth ───────────────────────────────────────────────────────────

export interface WalletAuthResponse {
  status: "active" | "provisioning";
  user: WalletAuthUser;
  wallet: WalletInfo | null;
}

export interface WalletAuthUser {
  id: string;
  email: string;
  riskTolerance: string;
  accountStatus: "active" | "pending_wallet";
}

export interface WalletSessionResponse {
  user: WalletAuthUser;
  wallet: WalletInfo | null;
  accountStatus: "active" | "pending_wallet";
}

export interface WalletAuthCodeResponse {
  challengeId: string;
  email: string;
  expiresAt: string;
  resendInSeconds: number;
  /** Present only in local dev when RESEND_API_KEY is not configured. */
  devCode?: string;
}

export const walletApi = {
  startEmail: (email: string, referrerHandle?: string) =>
    request<WalletAuthCodeResponse>("/auth/email/start", {
      method: "POST",
      body: { email, referrerHandle },
    }),
  verifyEmail: (
    challengeId: string,
    code: string,
    consent: {
      tos: boolean;
      privacy: boolean;
      tosVersion: string;
      privacyVersion: string;
      marketingOptIn: boolean;
    },
  ) =>
    request<WalletAuthResponse>("/auth/email/verify", {
      method: "POST",
      body: { challengeId, code, consent },
    }),
  resendEmail: (challengeId: string) =>
    request<WalletAuthCodeResponse>("/auth/email/resend", {
      method: "POST",
      body: { challengeId },
    }),
  session: () =>
    request<WalletSessionResponse>("/auth/session", { authed: true }),
  logout: async () => {
    const res = await fetch(`${BASE_URL}/auth/logout`, {
      method: "POST",
      headers: { "X-Aegis-Request": "1" },
      credentials: "include",
    });
    if (!res.ok) {
      const detail = await responseErrorDetail(res);
      throw new Error(`${res.status}: ${detail || "logout failed"}`);
    }
    clearRememberedAuthState();
  },
};

// ── Faucet ─────────────────────────────────────────────────────────────────

export interface FaucetClaim {
  amountUsdc: number;
  chain: string;
  txHash?: string | null;
  remainingTodayUsdc: number;
  claimedAt: string;
  /** Set in real mode — open this URL to complete the on-chain claim on
   *  Circle's public faucet. Null in mock mode (synthetic balance applied). */
  claimUrl?: string | null;
  /** ARC address the user should paste into the faucet. */
  arcAddress?: string | null;
}
export const faucetApi = {
  claim: () =>
    request<FaucetClaim>("/faucet/usdc", { method: "POST", authed: true }),
};

// ── Gateway ────────────────────────────────────────────────────────────────

export interface UnifiedBalance {
  unifiedUsdc: number;
  /** Sum of EURC across all chains. */
  unifiedEurc: number;
  /** USDC per chain — keys are lowercased chain shorthands ("arc", "base"). */
  perChain: Record<string, number>;
  /** EURC per chain — same key set as `perChain`. */
  perChainEurc: Record<string, number>;
  arcAddress?: string | null;
  baseAddress?: string | null;
}
export const gatewayApi = {
  balance: () => request<UnifiedBalance>("/gateway/balance", { authed: true }),
};

// ── Account ────────────────────────────────────────────────────────────────

export interface AccountExportResponse {
  status: "queued";
  deliveryEmail: string;
}

export interface DeleteAccountResponse {
  deletionRequestedAt: string;
  completesAt: string;
}

export const accountApi = {
  exportData: () =>
    request<AccountExportResponse>("/account/export", { authed: true }),
  deleteAccount: () =>
    request<DeleteAccountResponse>("/account/delete", {
      method: "POST",
      body: { confirm: true },
      authed: true,
    }),
};

// ── Portfolio ──────────────────────────────────────────────────────────────

export interface CreatePortfolioInput {
  name: string;
  allocations: Array<{
    symbol: string;
    quantity: number;
    targetWeight: number;
  }>;
  goal: PortfolioGoal;
}

export interface UpdatePortfolioInput {
  name?: string;
}

/** Wire shape of an allocation from the backend. The Rust serializer emits
 *  `assetSymbol` and `id` / `portfolioId`; the shared TS `Allocation` type
 *  uses `symbol` / `assetId`. We normalise to the shared shape inside
 *  `portfolioApi.get` so every consumer reads `symbol`. */
interface WireAllocation {
  id: string;
  portfolioId: string;
  assetSymbol: string;
  quantity: number;
  targetWeight: number;
  currentWeight: number;
  valueUsd: number;
}

export const portfolioApi = {
  list: async (): Promise<Portfolio[]> => {
    // The list endpoint intentionally omits allocation rows for speed. Keep
    // the shared Portfolio shape honest for UI code by normalising the field.
    const rows = await request<
      Array<Omit<Portfolio, "allocations"> & { allocations?: WireAllocation[] }>
    >("/portfolios", { authed: true });
    return rows.map((p) => ({
      ...p,
      allocations: (p.allocations ?? []).map(normalizeAllocation),
    }));
  },
  get: async (id: string): Promise<Portfolio> => {
    // The backend returns the full Portfolio inline with an `allocations`
    // array. Allocations come in with `assetSymbol`; normalise to `symbol`
    // so AllocationChart / AssetTable / RiskScoreCard can read them.
    const raw = await request<
      Omit<Portfolio, "allocations"> & { allocations: WireAllocation[] }
    >(`/portfolios/${id}`, { authed: true });
    return {
      ...raw,
      allocations: raw.allocations.map(normalizeAllocation),
    };
  },
  create: (payload: CreatePortfolioInput) =>
    request<Portfolio>("/portfolios", {
      method: "POST",
      body: payload,
      authed: true,
    }),
  update: (id: string, payload: UpdatePortfolioInput) =>
    request<Portfolio>(`/portfolios/${id}`, {
      method: "PUT",
      body: payload,
      authed: true,
    }),
  delete: (id: string) =>
    request<void>(`/portfolios/${id}`, { method: "DELETE", authed: true }),
  rebalance: (id: string) =>
    request<AgentDecision>(`/portfolios/${id}/rebalance`, {
      method: "POST",
      authed: true,
    }),
  getDiaryPublic: (id: string) =>
    request<{ id: string; diaryPublic: boolean }>(
      `/portfolios/${id}/diary-public`,
      { authed: true },
    ),
  setDiaryPublic: (id: string, diaryPublic: boolean) =>
    request<{ id: string; diaryPublic: boolean }>(
      `/portfolios/${id}/diary-public`,
      { method: "PATCH", body: { diaryPublic }, authed: true },
    ),
};

function normalizeAllocation(
  a: WireAllocation,
): Portfolio["allocations"][number] {
  return {
    assetId: a.id,
    symbol: a.assetSymbol as Portfolio["allocations"][number]["symbol"],
    quantity: a.quantity,
    targetWeight: a.targetWeight,
    currentWeight: a.currentWeight,
    valueUsd: a.valueUsd,
  };
}

// ── Market ─────────────────────────────────────────────────────────────────

export const marketApi = {
  snapshot: () => request<MarketSnapshot>("/market/snapshot"),
  prices: (symbols?: string[]) =>
    request<MarketSnapshot["assets"]>(
      `/market/prices${symbols ? `?symbols=${symbols.join(",")}` : ""}`,
    ),
};

// ── Agent ──────────────────────────────────────────────────────────────────

export const agentApi = {
  decisions: (portfolioId: string) =>
    request<AgentDecision[]>(`/agent/decisions/${portfolioId}`, {
      authed: true,
    }),
  decisionById: (decisionId: string) =>
    request<AgentDecision>(`/agent/decision/${decisionId}`, {
      authed: true,
    }),
  analyze: (portfolioId: string) =>
    request<AgentDecision>("/agent/analyze", {
      method: "POST",
      body: { portfolioId, triggeredBy: "user_request" },
      authed: true,
    }),
};

export interface AgentPauseStatus {
  pausedAt: string | null;
}

export const userAgentApi = {
  status: () => request<AgentPauseStatus>("/users/me/agent", { authed: true }),
  pause: () =>
    request<AgentPauseStatus>("/users/me/agent/pause", {
      method: "POST",
      authed: true,
    }),
  resume: () =>
    request<AgentPauseStatus>("/users/me/agent/resume", {
      method: "POST",
      authed: true,
    }),
};

// ── Strategies marketplace (SM-2) ─────────────────────────────────────────

export interface StrategyPublic {
  id: string;
  name: string;
  description: string;
  riskBand: "low" | "medium" | "high";
  minHorizonMonths: number;
  targetAllocation: Record<string, number>;
  isCurated: boolean;
  createdAt: string;
}

export interface AdoptResponse {
  portfolioId: string;
}

export const strategiesApi = {
  list: () => request<StrategyPublic[]>("/strategies"),
  get: (id: string) => request<StrategyPublic>(`/strategies/${id}`),
  adopt: (id: string) =>
    request<AdoptResponse>(`/strategies/${id}/adopt`, {
      method: "POST",
      authed: true,
    }),
};

// ── Rebalance ──────────────────────────────────────────────────────────────

export interface RebalancePlanResponse {
  rebalanceId: string;
  decisionId: string;
  executionMode?: "mock" | "real";
  totalLegs: number;
  legs: Array<{
    legIndex: number;
    kind: string;
    srcChain: ChainKey | null;
    destChain: ChainKey | null;
    srcSymbol: string | null;
    destSymbol: string | null;
    amountUsdc: number;
  }>;
}

export interface RebalanceApprovalSafety {
  approvable: boolean;
  code: string;
  message: string;
  missingCapabilities?: Array<{
    code: string;
    label: string;
    detail: string;
  }>;
}

export const rebalanceApi = {
  plan: (portfolioId: string) =>
    request<RebalancePlanResponse>(
      `/portfolios/${portfolioId}/rebalance/plan`,
      {
        method: "POST",
        authed: true,
      },
    ),
  execute: (rebalanceId: string) =>
    request<void>(`/rebalance/${rebalanceId}/execute`, {
      method: "POST",
      body: {},
      authed: true,
    }),
  get: (rebalanceId: string) =>
    request<{
      id: string;
      portfolioId: string;
      decisionId: string;
      status: string;
      executionMode?: "mock" | "real";
      totalLegs: number;
      completedLegs: number;
      totalGasUsdc: number | null;
      failureReason: string | null;
      approvedAt: string | null;
      completedAt: string | null;
      createdAt: string;
      updatedAt: string;
      approvalSafety: RebalanceApprovalSafety;
      protocolFeeSettlementTx?: string;
      legs: Array<{
        id: string;
        rebalanceId: string;
        legIndex: number;
        kind: string;
        srcChain: ChainKey | null;
        destChain: ChainKey | null;
        srcSymbol: string | null;
        destSymbol: string | null;
        amountUsdc: number;
        status: string;
        txHash: string | null;
        failureReason: string | null;
        submittedAt: string | null;
        confirmedAt: string | null;
      }>;
    }>(`/rebalance/${rebalanceId}`, { authed: true }),
  history: (portfolioId: string) =>
    request<
      Array<{
        id: string;
        portfolioId: string;
        decisionId: string;
        status: string;
        executionMode?: "mock" | "real";
        approvalSafety?: RebalanceApprovalSafety;
        totalAmountUsdc?: number;
        totalLegs: number;
        completedLegs: number;
        totalGasUsdc?: number | null;
        failureReason?: string | null;
        approvedAt?: string | null;
        completedAt?: string | null;
        createdAt: string;
        updatedAt: string;
      }>
    >(`/portfolios/${portfolioId}/rebalance/history`, { authed: true }),
};

// ── Peg defense (A6) ───────────────────────────────────────────────────────

export interface CreatePegRuleBody {
  portfolioId?: string | null;
  asset: PegAssetSymbol;
  thresholdPrice: number;
  windowSeconds?: number;
  actionKind: PegActionKind;
  targetAsset?: PegAssetSymbol | null;
}

export interface PatchPegRuleBody {
  enabled?: boolean;
  paused?: boolean;
  thresholdPrice?: number;
  windowSeconds?: number;
  actionKind?: PegActionKind;
  targetAsset?: PegAssetSymbol | null;
}

export const pegApi = {
  list: () => request<PegRule[]>("/peg/rules", { authed: true }),
  create: (body: CreatePegRuleBody) =>
    request<PegRule>("/peg/rules", {
      method: "POST",
      body,
      authed: true,
    }),
  patch: (id: string, body: PatchPegRuleBody) =>
    request<PegRule>(`/peg/rules/${id}`, {
      method: "PATCH",
      body,
      authed: true,
    }),
  remove: (id: string) =>
    request<void>(`/peg/rules/${id}`, { method: "DELETE", authed: true }),
  pause: (id: string) =>
    request<PegRule>(`/peg/rules/${id}/pause`, {
      method: "POST",
      authed: true,
    }),
  unpause: (id: string) =>
    request<PegRule>(`/peg/rules/${id}/unpause`, {
      method: "POST",
      authed: true,
    }),
};

// ── Tax ────────────────────────────────────────────────────────────────────

export interface TaxShareToken {
  id: string;
  portfolioId: string;
  year: number;
  token: string;
  shareUrl: string;
  expiresAt: string;
  revokedAt: string | null;
  createdAt: string;
}

export interface TaxShareCreated {
  tokenId: string;
  token: string;
  shareUrl: string;
  expiresAt: string;
}

export interface WalletSummaryRow {
  chain: string;
  address: string;
  lotCount: number;
  lastSyncedAt: string | null;
}

export interface TaxSummary {
  year: number;
  wallets: WalletSummaryRow[];
  totalLotCount: number;
  caveat: string;
}

export const taxApi = {
  harvestable: (portfolioId: string) =>
    request<HarvestableLoss[]>(`/tax/harvestable/${portfolioId}`, {
      authed: true,
    }),
  summary: (portfolioId: string, year: number) =>
    request<TaxSummary>(
      `/tax/summary?portfolioId=${portfolioId}&year=${year}`,
      { authed: true },
    ),
  /**
   * Trigger a CSV download via the browser. Bypasses the JSON `request`
   * helper because the response is a file, not JSON. Returns the number
   * of mock entries excluded (from X-Mock-Excluded) so the UI can render
   * a provenance line.
   */
  downloadCsv: async (
    portfolioId: string,
    year: number,
  ): Promise<{ mockExcluded: number }> => {
    const params = new URLSearchParams({ portfolioId, year: String(year) });
    const res = await fetch(`${BASE_URL}/tax/export.csv?${params}`, {
      credentials: "include",
    });
    if (!res.ok) {
      throw new Error(`${res.status}: ${res.statusText}`);
    }
    const blob = await res.blob();
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `aegis_tax_${year}_${portfolioId}.csv`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    window.URL.revokeObjectURL(url);
    const mockExcluded = Number(res.headers.get("X-Mock-Excluded") ?? "0");
    return { mockExcluded: Number.isFinite(mockExcluded) ? mockExcluded : 0 };
  },
  listShares: () => request<TaxShareToken[]>("/tax/shares", { authed: true }),
  createShare: (portfolioId: string, year: number, ttlDays: number) =>
    request<TaxShareCreated>("/tax/share", {
      method: "POST",
      body: { portfolioId, year, ttlDays },
      authed: true,
    }),
  revokeShare: (tokenId: string) =>
    request<void>(`/tax/share/${tokenId}`, {
      method: "DELETE",
      authed: true,
    }),
};

// ── Diary (public — no auth) ───────────────────────────────────────────────

export const diaryApi = {
  byWallet: (wallet: string) =>
    request<DiaryEntry[]>(`/diary/wallet/${wallet}`),
  byDecision: (decisionId: string) =>
    request<DiaryEntry>(`/diary/decision/${decisionId}`),
};

// ── Digest ─────────────────────────────────────────────────────────────────

export const digestApi = {
  subscribe: (email: string) =>
    request<{ unsubscribeToken: string }>("/digest/subscribe", {
      method: "POST",
      body: { email },
      authed: true,
    }),
  unsubscribe: () =>
    request<void>("/digest/subscribe", { method: "DELETE", authed: true }),
};

// ── Rates ──────────────────────────────────────────────────────────────────

export const ratesApi = {
  usycRate: () =>
    request<{
      annualizedYield: number;
      priceUsd: number;
      source: string;
      fetchedAt: string;
    }>("/treasury/usyc/rate"),
  usdcEurc: () =>
    request<{
      midRate: number;
      spreadBps: number;
      source: string;
      fetchedAt: string;
    }>("/fx/usdc-eurc"),
  paymasterEstimate: (chain: "arc" | "base", action: string) =>
    request<{
      chain: string;
      action: string;
      feeUsdc: number;
      via: string;
      isIndicative?: boolean;
    }>(
      `/paymaster/estimate?chain=${chain}&action=${encodeURIComponent(action)}`,
    ),
};

// ── Backtest preview ──────────────────────────────────────────────────────

export interface BacktestLegMetrics {
  totalReturnPct: number;
  sharpe: number;
  maxDrawdownPct: number;
  observations: number;
}

export interface BacktestResponse {
  current: BacktestLegMetrics;
  proposed: BacktestLegMetrics;
  deltaTotalReturnPct: number;
  windowDays: number;
  reliable: boolean;
}

export const backtestApi = {
  preview: (
    portfolioId: string,
    proposed?: Array<{ symbol: string; targetWeight: number }>,
  ) =>
    request<BacktestResponse>("/backtest/preview", {
      method: "POST",
      body: proposed ? { portfolioId, proposed } : { portfolioId },
      authed: true,
    }),
};

// ── Trustability + leaderboard ────────────────────────────────────────────

export interface TrustabilityRow {
  userId: string;
  handle: string;
  decisionsExecuted: number;
  distinctModels: number;
  avg7dReturn: number;
  trustabilityDelta: number;
  lastDecisionAt: string | null;
}

export interface TrustabilityResponse {
  row: TrustabilityRow | null;
  label: "excellent" | "strong" | "stable" | "shaky" | "underperforming" | null;
}

export const trustabilityApi = {
  me: () => request<TrustabilityResponse>("/trustability/me", { authed: true }),
};

export interface LeaderboardEntry extends TrustabilityRow {
  label: "excellent" | "strong" | "stable" | "shaky" | "underperforming";
}

export const leaderboardApi = {
  top: (limit = 50) =>
    request<LeaderboardEntry[]>(`/leaderboard?limit=${limit}`),
};

// ── Analytics (best-effort) ───────────────────────────────────────────────

export const analyticsApi = {
  track: async (
    eventName: string,
    properties: Record<string, unknown> = {},
  ): Promise<void> => {
    try {
      await request("/analytics/event", {
        method: "POST",
        body: { eventName, properties },
        authed: true,
      });
    } catch {
      /* analytics failures must never break user flows */
    }
  },
};

// ── Billing v2 ────────────────────────────────────────────────────────────

export interface ReferralRow {
  id: string;
  newUserId: string;
  rewardUsdc: number;
  paidAt: string | null;
  txHash: string | null;
  createdAt: string;
}

export interface ReferralsResponse {
  rows: ReferralRow[];
  totalPaidUsdc: number;
  totalPendingUsdc: number;
}

export interface PatchSubscriptionBody {
  /** ISO timestamp; when set, schedules cancellation at period end. */
  cancelAt?: string | null;
  /** Switch tier mid-cycle (proration handled server-side). */
  tier?: Tier;
}

export interface ListInvoicesParams {
  limit?: number;
  /** ISO timestamp cursor — return invoices created strictly before this. */
  before?: string;
}

export const billingApi = {
  /** Current user's active subscription. Returns `null` for Free users
   * who have never upgraded (server returns 204 / explicit null). */
  getSubscription: () =>
    request<Subscription | null>("/billing/subscription", { authed: true }),

  createSubscription: (payload: { tier: Tier }) =>
    request<Subscription>("/billing/subscriptions", {
      method: "POST",
      body: payload,
      authed: true,
    }),

  patchSubscription: (id: string, body: PatchSubscriptionBody) =>
    request<Subscription>(`/billing/subscriptions/${id}`, {
      method: "PATCH",
      body,
      authed: true,
    }),

  listInvoices: (params: ListInvoicesParams = {}) => {
    const qs = new URLSearchParams();
    if (params.limit !== undefined) qs.set("limit", String(params.limit));
    if (params.before) qs.set("before", params.before);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<Invoice[]>(`/billing/invoices${suffix}`, { authed: true });
  },

  /** Public — pricing page renders this for anonymous visitors. */
  listTiers: () => request<PricingTier[]>("/billing/tiers"),

  /** Referral earnings for the current user (GET /billing/referrals). */
  listReferrals: () =>
    request<ReferralsResponse>("/billing/referrals", { authed: true }),
};

// ── Health ─────────────────────────────────────────────────────────────────

export const healthApi = {
  check: () => request<{ status: string; version: string }>("/health"),
};
