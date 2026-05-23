import type { AgentDecision, Portfolio } from "@/types";

/**
 * Curated demo portfolios — used by `/explore/[portfolioId]`. Each one lands
 * in a different regime so visitors see distinct agent personalities without
 * signing up.
 */

export interface DemoBundle {
  portfolio: Portfolio;
  decisions: AgentDecision[];
  /**
   * Asset symbols that are illustrative only — the live route registry does
   * not yet execute them. The detail UI should badge these "coming soon".
   * Optional; absent means all sleeves in the portfolio are live-executable.
   */
  unsupportedSleeves?: string[];
}

const NOW = "2026-05-14T12:00:00Z";

function p(
  id: string,
  name: string,
  totalValueUsd: number,
  pnlPct: number,
  goal: Portfolio["goal"],
): Portfolio {
  return {
    id,
    userId: "demo",
    name,
    totalValueUsd,
    totalPnlUsd: (totalValueUsd * pnlPct) / 100,
    totalPnlPct: pnlPct,
    riskScore: 50,
    goal,
    createdAt: NOW,
    updatedAt: NOW,
    allocations: [],
  };
}

function d(
  id: string,
  portfolioId: string,
  reasoning: string,
  regime: "risk_on" | "neutral" | "risk_off",
  confidence: number,
  model: string,
  summary: string,
): AgentDecision {
  return {
    id,
    portfolioId,
    reasoning,
    recommendation: {
      summary,
      trades: [],
      expectedImpact: { riskDelta: 0, diversificationScore: 0.6 },
    },
    confidence,
    triggeredBy: "scheduled",
    createdAt: NOW,
    modelSlug: model,
    regime,
    promptTokens: 2400,
    completionTokens: 380,
    latencyMs: 4200,
    criticVerdict: {
      demandsRevision: false,
      notes: "Survives critique — proposal fits the user's horizon and regime.",
      confidence: 0.82,
    },
  };
}

export const DEMO_BUNDLES: Record<string, DemoBundle> = {
  "conservative-retiree": {
    portfolio: p("conservative-retiree", "Conservative Retiree", 125_000, 4.8, {
      name: "Conservative Retiree",
      objective: "preserve",
      horizon: "20y+",
      riskTolerance: "conservative",
      targetAllocation: { BTC: 20, ETH: 10, USYC: 60, EURC: 10 },
      includeUsyc: true,
      includeEurc: true,
      createdAt: NOW,
    }),
    decisions: [
      d(
        "demo-decision-cr-1",
        "conservative-retiree",
        "Vol-of-vol spiking; risk-off classifier confidence 0.81. In this simulated scenario the agent illustrates shifting further into a yield-bearing sleeve, keeping BTC drift under 25%. The holder's horizon (20y+) tolerates muted upside in exchange for drawdown protection. Note: USYC and EURC sleeves are coming soon — shown here as simulation only.",
        "risk_off",
        0.79,
        "anthropic/claude-opus-4-7",
        "Illustrative: increase yield sleeve; trim ETH back to target.",
      ),
    ],
    unsupportedSleeves: ["USYC", "EURC"],
  },
  "aggressive-builder": {
    portfolio: p("aggressive-builder", "Aggressive Builder", 48_000, 31.2, {
      name: "Aggressive Builder",
      objective: "grow",
      horizon: "5y",
      riskTolerance: "aggressive",
      targetAllocation: { BTC: 45, ETH: 35, SOL: 20 },
      includeUsyc: false,
      includeEurc: false,
      createdAt: NOW,
    }),
    decisions: [
      d(
        "demo-decision-ab-1",
        "aggressive-builder",
        "Risk-on regime; BTC dominance falling, alt strength widening. Holder's risk tolerance + 5y horizon supports letting SOL run past target before any trim. No reason to defend a static weight in a strong regime.",
        "risk_on",
        0.74,
        "anthropic/claude-opus-4-7",
        "Hold all — let SOL drift; revisit at +60% target weight.",
      ),
    ],
  },
  "operating-reserve": {
    portfolio: p("operating-reserve", "Operating Reserve", 2_400_000, -1.4, {
      name: "Operating Reserve",
      objective: "income",
      horizon: "3y",
      riskTolerance: "moderate",
      targetAllocation: { BTC: 25, ETH: 15, USYC: 40, EURC: 20 },
      includeUsyc: true,
      includeEurc: true,
      createdAt: NOW,
    }),
    decisions: [
      d(
        "demo-decision-td-1",
        "operating-reserve",
        "Neutral regime with correlation ticking up. In this simulated scenario the agent illustrates a USYC yield sleeve and an EURC sleeve to diversify operating-currency exposure. Note: USYC and EURC sleeves are coming soon — shown here as simulation only.",
        "neutral",
        0.81,
        "anthropic/claude-opus-4-7",
        "Hold; illustrative: sweep idle USDC into yield sleeve (coming soon).",
      ),
    ],
    unsupportedSleeves: ["USYC", "EURC"],
  },
};

export const DEMO_SLUGS = Object.keys(DEMO_BUNDLES);
