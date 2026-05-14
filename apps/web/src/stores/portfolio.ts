import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type {
  Portfolio,
  PortfolioId,
  AgentDecision,
  MarketSnapshot,
  MarketRegime,
  RegimeSignals,
  PriceTick,
  WalletInfo,
} from "@/types";
import {
  MOCK_PORTFOLIO,
  MOCK_AGENT_DECISIONS,
  MOCK_MARKET_SNAPSHOT,
} from "@/lib/mock-data";

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

  setPortfolios: (p: Portfolio[]) => void;
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
  initMockData: () => void;
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

      setPortfolios: (portfolios) =>
        set((state) => ({
          portfolios,
          activePortfolioId:
            state.activePortfolioId ?? portfolios[0]?.id ?? null,
        })),
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

      initMockData: () =>
        set({
          portfolios: [MOCK_PORTFOLIO],
          activePortfolioId: MOCK_PORTFOLIO.id,
          decisions: MOCK_AGENT_DECISIONS,
          marketSnapshot: MOCK_MARKET_SNAPSHOT,
        }),
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
