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
export const HARVEST_THRESHOLD_USD = 50; // strategist gets a harvest signal when open losses exceed this

// ── Chain address book ─────────────────────────────────────────────────────
//
// Addresses below are the published testnet artifacts at the time of the
// hackathon. `REBALANCE_EXECUTOR` slots are filled by `infra/contracts`'s
// Deploy script and overwritten in this file as part of S3.5. Mainnet entries
// stay empty until a production deploy is on the table.

// Verified against deployed MessageTransmitter.localDomain() on each
// chain. Arc testnet returns 26, not 13 — the latter was a stale guess
// that surfaced only when an attested CCTP V2 message reverted with
// "Invalid destination domain" on Arc.
export const CHAIN_DOMAINS = {
  arc: 26,
  base: 6,
} as const;

export interface ChainAddressBook {
  usdc: `0x${string}`;
  cctpV2TokenMessenger: `0x${string}`;
  cctpV2MessageTransmitter: `0x${string}`;
  rebalanceExecutor: `0x${string}` | null;
  uniswapV3SwapRouter: `0x${string}` | null;
}

export const CHAIN_ADDRESSES: Record<"arc" | "base", ChainAddressBook> = {
  arc: {
    usdc: "0x0000000000000000000000000000000000000000",
    cctpV2TokenMessenger: "0x0000000000000000000000000000000000000000",
    cctpV2MessageTransmitter: "0x0000000000000000000000000000000000000000",
    rebalanceExecutor: null,
    uniswapV3SwapRouter: null,
  },
  base: {
    usdc: "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
    cctpV2TokenMessenger: "0x8FE6B999Dc680CcFDD5Bf7EB0974218be2542DAA",
    cctpV2MessageTransmitter: "0xE737e5cEBEEBa77EFE34D4aa090756590b1CE275",
    rebalanceExecutor: null,
    uniswapV3SwapRouter: "0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4",
  },
} as const;

export const USYC_ADDRESS_ARC = "0x0000000000000000000000000000000000000000";

export const API_ROUTES = {
  health: "/health",
  auth: {
    startEmail: "/auth/email/start",
    verifyEmail: "/auth/email/verify",
    session: "/auth/session",
    logout: "/auth/logout",
  },
  portfolios: {
    list: "/portfolios",
    create: "/portfolios",
    get: (id: string) => `/portfolios/${id}`,
    update: (id: string) => `/portfolios/${id}`,
    delete: (id: string) => `/portfolios/${id}`,
    rebalance: (id: string) => `/portfolios/${id}/rebalance`,
    rebalancePlan: (id: string) => `/portfolios/${id}/rebalance/plan`,
    rebalanceHistory: (id: string) => `/portfolios/${id}/rebalance/history`,
  },
  market: {
    prices: "/market/prices",
    snapshot: "/market/snapshot",
  },
  agent: {
    decisions: (portfolioId: string) => `/agent/decisions/${portfolioId}`,
    analyze: "/agent/analyze",
  },
  rebalance: {
    get: (id: string) => `/rebalance/${id}`,
    execute: (id: string) => `/rebalance/${id}/execute`,
  },
  tax: {
    harvestable: (portfolioId: string) => `/tax/harvestable/${portfolioId}`,
  },
  digest: {
    subscribe: "/digest/subscribe",
    unsubscribe: "/digest/unsubscribe",
  },
  diary: {
    decision: (id: string) => `/diary/decision/${id}`,
    wallet: (addr: string) => `/diary/wallet/${addr}`,
  },
} as const;
