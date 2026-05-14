import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type {
  Portfolio,
  AgentDecision,
  MarketSnapshot,
  MarketRegime,
  RegimeSignals,
  PriceTick,
} from "@/types";
import { MOCK_PORTFOLIO, MOCK_AGENT_DECISIONS, MOCK_MARKET_SNAPSHOT } from "@/lib/mock-data";

export interface RegimeState {
  current: MarketRegime;
  previous: MarketRegime | null;
  confidence: number;
  signals: RegimeSignals | null;
  classifiedAt: string | null;
}

interface PortfolioState {
  portfolio: Portfolio | null;
  decisions: AgentDecision[];
  marketSnapshot: MarketSnapshot | null;
  regime: RegimeState;
  /** Latest price per symbol — kept fresh by SSE `price.tick` events. */
  livePrices: Record<string, PriceTick>;
  isRebalancing: boolean;
  selectedDecisionId: string | null;
  sseConnected: boolean;

  setPortfolio: (p: Portfolio) => void;
  setDecisions: (d: AgentDecision[]) => void;
  addDecision: (d: AgentDecision) => void;
  setMarketSnapshot: (s: MarketSnapshot) => void;
  setRegime: (next: Partial<RegimeState>) => void;
  applyPriceTick: (tick: PriceTick) => void;
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
      portfolio: null,
      decisions: [],
      marketSnapshot: null,
      regime: DEFAULT_REGIME,
      livePrices: {},
      isRebalancing: false,
      selectedDecisionId: null,
      sseConnected: false,

      setPortfolio: (portfolio) => set({ portfolio }),
      setDecisions: (decisions) => set({ decisions }),
      addDecision: (decision) =>
        set((state) => ({
          // Deduplicate by id so a re-fired SSE event doesn't double-insert.
          decisions: [decision, ...state.decisions.filter((d) => d.id !== decision.id)].slice(
            0,
            100
          ),
        })),
      setMarketSnapshot: (marketSnapshot) => set({ marketSnapshot }),
      setRegime: (next) =>
        set((state) => ({ regime: { ...state.regime, ...next } })),
      applyPriceTick: (tick) =>
        set((state) => ({
          livePrices: { ...state.livePrices, [tick.symbol]: tick },
        })),
      setIsRebalancing: (isRebalancing) => set({ isRebalancing }),
      selectDecision: (selectedDecisionId) => set({ selectedDecisionId }),
      setSseConnected: (sseConnected) => set({ sseConnected }),

      initMockData: () =>
        set({
          portfolio: MOCK_PORTFOLIO,
          decisions: MOCK_AGENT_DECISIONS,
          marketSnapshot: MOCK_MARKET_SNAPSHOT,
        }),
    }),
    { name: "aegis-portfolio" }
  )
);
