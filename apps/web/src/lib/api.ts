import type {
  AgentDecision,
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
  createPasskey: (email: string, passkeyAttestation: unknown) =>
    request<WalletAuthResponse>("/auth/wallet/create", {
      method: "POST",
      body: { email, passkeyAttestation },
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
  verifyOtp: (email: string, code: string) =>
    request<WalletAuthResponse>("/auth/wallet/otp/verify", {
      method: "POST",
      body: { email, code },
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
  analyze: (portfolioId: string) =>
    request<AgentDecision>("/agent/analyze", {
      method: "POST",
      body: { portfolioId, triggeredBy: "user_request" },
      authed: true,
    }),
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
