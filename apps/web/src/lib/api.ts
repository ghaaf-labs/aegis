import type {
  AgentDecision,
  DiaryEntry,
  HarvestableLoss,
  MarketSnapshot,
  Portfolio,
  PortfolioGoal,
  WalletInfo,
} from "@/types";

const BASE_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

const TOKEN_KEY = "aegis.jwt";

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(TOKEN_KEY);
}

export function setToken(t: string | null) {
  if (typeof window === "undefined") return;
  if (t) window.localStorage.setItem(TOKEN_KEY, t);
  else window.localStorage.removeItem(TOKEN_KEY);
}

interface FetchOptions {
  method?: string;
  body?: unknown;
  authed?: boolean;
}

async function request<T>(path: string, opts: FetchOptions = {}): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  const token = opts.authed ? getToken() : null;
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(`${BASE_URL}${path}`, {
    method: opts.method ?? "GET",
    headers,
    // `credentials: 'include'` ensures the httpOnly auth cookie set by the
    // wallet endpoints rides on every cross-origin request. Backend CORS
    // enables `Access-Control-Allow-Credentials` with a specific origin
    // allow-list (no wildcard).
    credentials: "include",
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
  });
  if (!res.ok) {
    let detail = res.statusText;
    try {
      const body = await res.json();
      detail = body.message ?? body.code ?? detail;
    } catch {
      /* ignore */
    }
    throw new Error(`${res.status}: ${detail}`);
  }
  return res.json() as Promise<T>;
}

// ── Wallet auth (replaces legacy email/password authApi) ──────────────────

export interface WalletAuthResponse {
  token: string;
  wallet: WalletInfo;
  user: { id: string; email: string; riskTolerance: string };
}

export const walletApi = {
  createPasskey: (
    email: string,
    passkeyAttestation: unknown,
    referrerHandle?: string,
  ) =>
    request<WalletAuthResponse>("/auth/wallet/create", {
      method: "POST",
      body: { email, passkeyAttestation, referrerHandle },
    }),
  loginPasskey: (email: string, passkeyAssertion: unknown) =>
    request<WalletAuthResponse>("/auth/wallet/login", {
      method: "POST",
      body: { email, passkeyAssertion },
    }),
  startOtp: (email: string) =>
    request<{ email: string; challengeId: string; expiresIn: number }>(
      "/auth/wallet/otp/start",
      { method: "POST", body: { email } },
    ),
  verifyOtp: (email: string, code: string, referrerHandle?: string) =>
    request<WalletAuthResponse>("/auth/wallet/otp/verify", {
      method: "POST",
      body: { email, code, referrerHandle },
    }),
  me: () =>
    request<{ id: string; email: string; riskTolerance: string }>("/auth/me", {
      authed: true,
    }),
};

// ── Faucet ─────────────────────────────────────────────────────────────────

export interface FaucetClaim {
  amountUsdc: number;
  chain: string;
  txHash?: string | null;
  remainingTodayUsdc: number;
  claimedAt: string;
}
export const faucetApi = {
  claim: () =>
    request<FaucetClaim>("/faucet/usdc", { method: "POST", authed: true }),
};

// ── Gateway ────────────────────────────────────────────────────────────────

export interface UnifiedBalance {
  unifiedUsdc: number;
  perChain: Record<string, number>;
  arcAddress?: string | null;
  baseAddress?: string | null;
}
export const gatewayApi = {
  balance: () => request<UnifiedBalance>("/gateway/balance", { authed: true }),
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

export const portfolioApi = {
  list: () => request<Portfolio[]>("/portfolios", { authed: true }),
  get: (id: string) =>
    request<{
      portfolio: Portfolio;
      allocations: Array<{
        assetId: string;
        symbol: string;
        quantity: number;
        targetWeight: number;
        currentWeight: number;
        valueUsd: number;
      }>;
    }>(`/portfolios/${id}`, { authed: true }),
  create: (payload: CreatePortfolioInput) =>
    request<Portfolio>("/portfolios", {
      method: "POST",
      body: payload,
      authed: true,
    }),
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

// ── Rebalance ──────────────────────────────────────────────────────────────

export interface RebalancePlanResponse {
  rebalanceId: string;
  decisionId: string;
  totalLegs: number;
  legs: Array<{
    legIndex: number;
    kind: string;
    srcChain: string | null;
    destChain: string | null;
    srcSymbol: string | null;
    destSymbol: string | null;
    amountUsdc: number;
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
      totalLegs: number;
      completedLegs: number;
      totalGasUsdc: number | null;
      failureReason: string | null;
      approvedAt: string | null;
      completedAt: string | null;
      createdAt: string;
      updatedAt: string;
      protocolFeeSettlementTx?: string;
      legs: Array<{
        id: string;
        rebalanceId: string;
        legIndex: number;
        kind: string;
        srcChain: string | null;
        destChain: string | null;
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
        status: string;
        totalLegs: number;
        completedLegs: number;
        createdAt: string;
      }>
    >(`/portfolios/${portfolioId}/rebalance/history`, { authed: true }),
};

// ── Tax ────────────────────────────────────────────────────────────────────

export const taxApi = {
  harvestable: (portfolioId: string) =>
    request<HarvestableLoss[]>(`/tax/harvestable/${portfolioId}`, {
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

// ── Health ─────────────────────────────────────────────────────────────────

export const healthApi = {
  check: () => request<{ status: string; version: string }>("/health"),
};
