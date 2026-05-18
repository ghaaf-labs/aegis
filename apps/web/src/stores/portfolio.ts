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
  activePortfolioId: PortfolioId | null;
  decisions: AgentDecision[];
  marketSnapshot: MarketSnapshot | null;
  regime: RegimeState;
  /** Latest price per symbol — kept fresh by SSE `price.tick` events. */
  livePrices: Record<string, PriceTick>;
  isRebalancing: boolean;
  selectedDecisionId: string | null;
  sseConnected: boolean;
  /** Wallet info from Circle Wallets create / login. */
  wallet: WalletInfo | null;
  unifiedUsdc: number;
  /** Most-recent strategist tool invocations (capped at 20). */
  toolInvocations: AgentToolInvoked[];
  /** Most-recent abstain events (capped at 10). */
  abstains: AgentAbstained[];
  /** Latest status update per rebalance id — kept fresh by SSE. */
  rebalanceStatuses: Record<string, RebalanceStatus>;
  /** Most-recent peg-alert events (capped at 20). */
  pegAlerts: PegAlert[];

  setPortfolios: (p: Portfolio[]) => void;
  setPortfoliosLoaded: (v: boolean) => void;
  addPortfolio: (p: Portfolio) => void;
  setActivePortfolio: (id: PortfolioId | null) => void;
  setDecisions: (d: AgentDecision[]) => void;
  addDecision: (d: AgentDecision) => void;
  setMarketSnapshot: (s: MarketSnapshot) => void;
  setRegime: (next: Partial<RegimeState>) => void;
  applyPriceTick: (tick: PriceTick) => void;
  setUnifiedUsdc: (v: number) => void;
  setWallet: (w: WalletInfo | null) => void;
  setIsRebalancing: (v: boolean) => void;
  selectDecision: (id: string | null) => void;
  setSseConnected: (v: boolean) => void;
  pushToolInvocation: (t: AgentToolInvoked) => void;
  pushAbstain: (a: AgentAbstained) => void;
  applyRebalanceStatus: (s: RebalanceStatus) => void;
  pushPegAlert: (a: PegAlert) => void;
}

const DEFAULT_REGIME: RegimeState = {
  current: "neutral",
  previous: null,
  confidence: 0,
  signals: null,
  classifiedAt: null,
};

export const usePortfolioStore = create<PortfolioState>()(
  devtools(
    (set) => ({
      portfolios: [],
      portfoliosLoaded: false,
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
      toolInvocations: [],
      abstains: [],
      rebalanceStatuses: {},
      pegAlerts: [],

      setPortfolios: (portfolios) =>
        set((state) => ({
          portfolios,
          portfoliosLoaded: true,
          activePortfolioId:
            state.activePortfolioId ?? portfolios[0]?.id ?? null,
        })),
      setPortfoliosLoaded: (portfoliosLoaded) => set({ portfoliosLoaded }),
      addPortfolio: (portfolio) =>
        set((state) => ({
          portfolios: [
            ...state.portfolios.filter((p) => p.id !== portfolio.id),
            portfolio,
          ],
          activePortfolioId: portfolio.id,
        })),
      setActivePortfolio: (activePortfolioId) => set({ activePortfolioId }),
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
