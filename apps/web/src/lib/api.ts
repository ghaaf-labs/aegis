import type { ApiResponse, Portfolio, AgentDecision, MarketSnapshot } from "@/types";

const BASE_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, {
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
    ...init,
  });

  if (!res.ok) {
    const error = await res.json().catch(() => ({ message: "Request failed" }));
    throw new Error(error.message ?? `HTTP ${res.status}`);
  }

  return res.json() as Promise<T>;
}

// ── Auth ───────────────────────────────────────────────────────────────────

export interface LoginPayload { email: string; password: string }
export interface AuthResponse { token: string; user: { id: string; email: string } }

export const authApi = {
  login: (payload: LoginPayload) =>
    request<AuthResponse>("/auth/login", { method: "POST", body: JSON.stringify(payload) }),
  register: (payload: LoginPayload & { riskTolerance: string }) =>
    request<AuthResponse>("/auth/register", { method: "POST", body: JSON.stringify(payload) }),
  me: (token: string) =>
    request<AuthResponse["user"]>("/auth/me", { headers: { Authorization: `Bearer ${token}` } }),
};

// ── Portfolio ──────────────────────────────────────────────────────────────

export const portfolioApi = {
  list: () => request<ApiResponse<Portfolio[]>>("/portfolios"),
  get: (id: string) => request<Portfolio>(`/portfolios/${id}`),
  create: (payload: Partial<Portfolio>) =>
    request<Portfolio>("/portfolios", { method: "POST", body: JSON.stringify(payload) }),
  rebalance: (id: string) =>
    request<AgentDecision>(`/portfolios/${id}/rebalance`, { method: "POST" }),
};

// ── Market ─────────────────────────────────────────────────────────────────

export const marketApi = {
  snapshot: () => request<MarketSnapshot>("/market/snapshot"),
  prices: (symbols?: string[]) =>
    request<MarketSnapshot["assets"]>(
      `/market/prices${symbols ? `?symbols=${symbols.join(",")}` : ""}`
    ),
};

// ── Agent ──────────────────────────────────────────────────────────────────

export const agentApi = {
  decisions: (portfolioId: string) =>
    request<ApiResponse<AgentDecision[]>>(`/agent/decisions/${portfolioId}`),
  analyze: (portfolioId: string) =>
    request<AgentDecision>("/agent/analyze", {
      method: "POST",
      body: JSON.stringify({ portfolioId }),
    }),
};

// ── Health ─────────────────────────────────────────────────────────────────

export const healthApi = {
  check: () => request<{ status: string; version: string }>("/health"),
};
