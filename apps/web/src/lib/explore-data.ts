import type { AgentDecision, Portfolio } from "@/types";

/**
 * Curated demo portfolios — used by `/explore/[portfolioId]`. Each one lands
 * in a different regime so visitors see distinct agent personalities without
 * signing up.
 */

export interface DemoBundle {
  portfolio: Portfolio;
  decisions: AgentDecision[];
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
        "Vol-of-vol spiking; risk-off classifier confidence 0.81. Lean further into USYC, keep BTC drift under 25%. Holder's horizon (20y+) tolerates the muted upside in exchange for the drawdown protection USYC offers.",
        "risk_off",
        0.79,
        "anthropic/claude-opus-4-7",
        "Park 5% more in USYC; trim ETH back to target.",
      ),
    ],
  },
  "aggressive-builder": {
    portfolio: p("aggressive-builder", "Aggressive Builder", 48_000, 31.2, {
      name: "Aggressive Builder",
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
        "Neutral regime with correlation ticking up; cash earning 5.1% on USYC is hard to beat at this risk level. Add an EURC sleeve to diversify operating-currency exposure.",
        "neutral",
        0.81,
        "anthropic/claude-opus-4-7",
        "Hold; sweep idle USDC into USYC weekly.",
      ),
    ],
  },
};

export const DEMO_SLUGS = Object.keys(DEMO_BUNDLES);
