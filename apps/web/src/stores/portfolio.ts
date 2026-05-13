import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type { Portfolio, AgentDecision, MarketSnapshot } from "@/types";
import { MOCK_PORTFOLIO, MOCK_AGENT_DECISIONS, MOCK_MARKET_SNAPSHOT } from "@/lib/mock-data";

interface PortfolioState {
  portfolio: Portfolio | null;
  decisions: AgentDecision[];
  marketSnapshot: MarketSnapshot | null;
  isRebalancing: boolean;
  selectedDecisionId: string | null;

  setPortfolio: (p: Portfolio) => void;
  setDecisions: (d: AgentDecision[]) => void;
  addDecision: (d: AgentDecision) => void;
  setMarketSnapshot: (s: MarketSnapshot) => void;
  setIsRebalancing: (v: boolean) => void;
  selectDecision: (id: string | null) => void;
  initMockData: () => void;
}

export const usePortfolioStore = create<PortfolioState>()(
  devtools(
    (set) => ({
      portfolio: null,
      decisions: [],
      marketSnapshot: null,
      isRebalancing: false,
      selectedDecisionId: null,

      setPortfolio: (portfolio) => set({ portfolio }),
      setDecisions: (decisions) => set({ decisions }),
      addDecision: (decision) =>
        set((state) => ({ decisions: [decision, ...state.decisions] })),
      setMarketSnapshot: (marketSnapshot) => set({ marketSnapshot }),
      setIsRebalancing: (isRebalancing) => set({ isRebalancing }),
      selectDecision: (selectedDecisionId) => set({ selectedDecisionId }),

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
