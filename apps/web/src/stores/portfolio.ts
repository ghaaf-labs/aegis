import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type {
  Portfolio,
  PortfolioId,
  AgentDecision,
  AgentToolInvoked,
  AgentAbstained,
  MarketSnapshot,
  MarketRegime,
  RegimeSignals,
  PriceTick,
  WalletInfo,
  RebalanceStatus,
  PegAlert,
} from "@/types";
export interface RegimeState {
  current: MarketRegime;
  previous: MarketRegime | null;
  confidence: number;
  signals: RegimeSignals | null;
  classifiedAt: string | null;
}

interface PortfolioState {
  /** All portfolios for the logged-in user. */
  portfolios: Portfolio[];
  /** True once portfolioApi.list() has resolved at least once this session. */
  portfoliosLoaded: boolean;
  /** True when the last portfolios fetch failed (distinct from "loaded empty"). */
  portfoliosError: boolean;
  activePortfolioId: PortfolioId | null;
  decisions: AgentDecision[];
  marketSnapshot: MarketSnapshot | null;
  regime: RegimeState;
  /** Latest price per symbol — kept fresh by SSE `price.tick` events. */
  livePrices: Record<string, PriceTick>;
  isRebalancing: boolean;
  selectedDecisionId: string | null;
  sseConnected: boolean;
  /** Wallet info returned after email verification and account restore. */
  wallet: WalletInfo | null;
  unifiedUsdc: number;
  /** Sum of EURC across every chain the user holds a wallet on. */
  unifiedEurc: number;
  /** USDC per wallet route (for example "arc", "base", "eth-sepolia"). */
  perChainUsdc: Record<string, number>;
  /** EURC per chain — same key set as perChainUsdc. */
  perChainEurc: Record<string, number>;
  /** Whether the latest wallet cash balance fetch is known-good. */
  gatewayBalanceStatus: "idle" | "loading" | "ready" | "error";
  gatewayBalanceError: string | null;
  /** Most-recent strategist tool invocations (capped at 20). */
  toolInvocations: AgentToolInvoked[];
  /** Most-recent abstain events (capped at 10). */
  abstains: AgentAbstained[];
  /** Latest status update per rebalance id — kept fresh by SSE. */
  rebalanceStatuses: Record<string, RebalanceStatus>;
  /** Most-recent peg-alert events (capped at 20). */
  pegAlerts: PegAlert[];
  /** Global scheduled-agent pause timestamp. Null means scheduled triggers run. */
  agentPausedAt: string | null;
  /** True after `/auth/session` confirms an active session. */
  sessionActive: boolean;
  /** True once the initial `/auth/session` probe has settled this page load. */
  sessionResolved: boolean;

  setPortfolios: (p: Portfolio[]) => void;
  setPortfoliosLoaded: (v: boolean) => void;
  setPortfoliosError: (v: boolean) => void;
  /** Merge a partial update into the portfolio with the given id. Used by
   * the dashboard to layer allocations from `/portfolios/:id` onto the
   * list-shape entry from `/portfolios`. */
  patchPortfolio: (id: PortfolioId, patch: Partial<Portfolio>) => void;
  addPortfolio: (p: Portfolio) => void;
  setActivePortfolio: (id: PortfolioId | null) => void;
  setDecisions: (d: AgentDecision[]) => void;
  addDecision: (d: AgentDecision) => void;
  setMarketSnapshot: (s: MarketSnapshot) => void;
  setRegime: (next: Partial<RegimeState>) => void;
  applyPriceTick: (tick: PriceTick) => void;
  setUnifiedUsdc: (v: number) => void;
  setUnifiedEurc: (v: number) => void;
  setPerChain: (
    usdc: Record<string, number>,
    eurc: Record<string, number>,
  ) => void;
  setGatewayBalanceStatus: (
    status: "idle" | "loading" | "ready" | "error",
    error?: string | null,
  ) => void;
  setWallet: (w: WalletInfo | null) => void;
  setIsRebalancing: (v: boolean) => void;
  selectDecision: (id: string | null) => void;
  setSseConnected: (v: boolean) => void;
  pushToolInvocation: (t: AgentToolInvoked) => void;
  pushAbstain: (a: AgentAbstained) => void;
  applyRebalanceStatus: (s: RebalanceStatus) => void;
  pushPegAlert: (a: PegAlert) => void;
  setAgentPausedAt: (pausedAt: string | null) => void;
  setSessionActive: (active: boolean) => void;
  setSessionResolved: (resolved: boolean) => void;
  resetSession: () => void;
}

const DEFAULT_REGIME: RegimeState = {
  current: "neutral",
  previous: null,
  confidence: 0,
  signals: null,
  classifiedAt: null,
};

const ACTIVE_PORTFOLIO_KEY = "aegis.active_portfolio_id";

export const usePortfolioStore = create<PortfolioState>()(
  devtools(
    (set) => ({
      portfolios: [],
      portfoliosLoaded: false,
      portfoliosError: false,
      activePortfolioId: null,
      decisions: [],
      marketSnapshot: null,
      regime: DEFAULT_REGIME,
      livePrices: {},
      isRebalancing: false,
      selectedDecisionId: null,
      sseConnected: false,
      wallet: null,
      unifiedUsdc: 0,
      unifiedEurc: 0,
      perChainUsdc: {},
      perChainEurc: {},
      gatewayBalanceStatus: "idle",
      gatewayBalanceError: null,
      toolInvocations: [],
      abstains: [],
      rebalanceStatuses: {},
      pegAlerts: [],
      agentPausedAt: null,
      sessionActive: false,
      sessionResolved: false,

      setPortfolios: (portfolios) =>
        set((state) => {
          const existingById = new Map(state.portfolios.map((p) => [p.id, p]));
          const mergedPortfolios = portfolios.map((portfolio) => {
            const existing = existingById.get(portfolio.id);
            const existingAllocations = existing?.allocations;
            if (
              (portfolio.allocations?.length ?? 0) === 0 &&
              (existingAllocations?.length ?? 0) > 0
            ) {
              return { ...portfolio, allocations: existingAllocations ?? [] };
            }
            return portfolio;
          });
          const preferred =
            state.activePortfolioId ?? loadStoredActivePortfolioId();
          const activePortfolioId = mergedPortfolios.some(
            (p) => p.id === preferred,
          )
            ? preferred
            : (mergedPortfolios[0]?.id ?? null);
          saveStoredActivePortfolioId(activePortfolioId);
          return {
            portfolios: mergedPortfolios,
            portfoliosLoaded: true,
            portfoliosError: false,
            activePortfolioId,
          };
        }),
      setPortfoliosLoaded: (portfoliosLoaded) => set({ portfoliosLoaded }),
      setPortfoliosError: (portfoliosError) => set({ portfoliosError }),
      patchPortfolio: (id, patch) =>
        set((state) => ({
          portfolios: state.portfolios.map((p) =>
            p.id === id ? { ...p, ...patch } : p,
          ),
        })),
      addPortfolio: (portfolio) =>
        set((state) => {
          saveStoredActivePortfolioId(portfolio.id);
          return {
            portfolios: [
              ...state.portfolios.filter((p) => p.id !== portfolio.id),
              portfolio,
            ],
            portfoliosLoaded: true,
            portfoliosError: false,
            activePortfolioId: portfolio.id,
          };
        }),
      setActivePortfolio: (activePortfolioId) => {
        saveStoredActivePortfolioId(activePortfolioId);
        set({ activePortfolioId });
      },
      setDecisions: (decisions) => set({ decisions }),
      addDecision: (decision) =>
        set((state) => ({
          decisions: [
            decision,
            ...state.decisions.filter((d) => d.id !== decision.id),
          ].slice(0, 100),
        })),
      setMarketSnapshot: (marketSnapshot) => set({ marketSnapshot }),
      setRegime: (next) =>
        set((state) => ({ regime: { ...state.regime, ...next } })),
      applyPriceTick: (tick) =>
        set((state) => ({
          livePrices: { ...state.livePrices, [tick.symbol]: tick },
        })),
      setUnifiedUsdc: (unifiedUsdc) => set({ unifiedUsdc }),
      setUnifiedEurc: (unifiedEurc) => set({ unifiedEurc }),
      setPerChain: (perChainUsdc, perChainEurc) =>
        set({ perChainUsdc, perChainEurc }),
      setGatewayBalanceStatus: (gatewayBalanceStatus, gatewayBalanceError) =>
        set({
          gatewayBalanceStatus,
          gatewayBalanceError:
            gatewayBalanceStatus === "error"
              ? (gatewayBalanceError ?? "Wallet balance unavailable")
              : null,
        }),
      setWallet: (wallet) => set({ wallet }),
      setIsRebalancing: (isRebalancing) => set({ isRebalancing }),
      selectDecision: (selectedDecisionId) => set({ selectedDecisionId }),
      setSseConnected: (sseConnected) => set({ sseConnected }),
      pushToolInvocation: (invocation) =>
        set((state) => ({
          toolInvocations: [invocation, ...state.toolInvocations].slice(0, 20),
        })),
      pushAbstain: (abstain) =>
        set((state) => ({
          abstains: [abstain, ...state.abstains].slice(0, 10),
        })),
      applyRebalanceStatus: (status) =>
        set((state) => ({
          rebalanceStatuses: {
            ...state.rebalanceStatuses,
            [status.id]: status,
          },
        })),
      pushPegAlert: (alert) =>
        set((state) => ({
          pegAlerts: [alert, ...state.pegAlerts].slice(0, 20),
        })),
      setAgentPausedAt: (agentPausedAt) => set({ agentPausedAt }),
      setSessionActive: (sessionActive) => set({ sessionActive }),
      setSessionResolved: (sessionResolved) => set({ sessionResolved }),
      resetSession: () => {
        saveStoredActivePortfolioId(null);
        set({
          portfolios: [],
          portfoliosLoaded: false,
          portfoliosError: false,
          activePortfolioId: null,
          decisions: [],
          wallet: null,
          unifiedUsdc: 0,
          unifiedEurc: 0,
          perChainUsdc: {},
          perChainEurc: {},
          gatewayBalanceStatus: "idle",
          gatewayBalanceError: null,
          isRebalancing: false,
          selectedDecisionId: null,
          sseConnected: false,
          toolInvocations: [],
          abstains: [],
          rebalanceStatuses: {},
          pegAlerts: [],
          agentPausedAt: null,
          sessionActive: false,
          sessionResolved: false,
        });
      },
    }),
    { name: "aegis-portfolio" },
  ),
);

/** Helper to read the active portfolio (or null). */
export function useActivePortfolio(): Portfolio | null {
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const active = usePortfolioStore((s) => s.activePortfolioId);
  return portfolios.find((p) => p.id === active) ?? null;
}

function loadStoredActivePortfolioId(): PortfolioId | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(ACTIVE_PORTFOLIO_KEY);
}

function saveStoredActivePortfolioId(id: PortfolioId | null) {
  if (typeof window === "undefined") return;
  if (id) {
    window.localStorage.setItem(ACTIVE_PORTFOLIO_KEY, id);
  } else {
    window.localStorage.removeItem(ACTIVE_PORTFOLIO_KEY);
  }
}
