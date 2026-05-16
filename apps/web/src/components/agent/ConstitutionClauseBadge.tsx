"use client";

import { BrutalPill } from "@aegis/ui";

const CLAUSE_SUMMARIES: Record<string, string> = {
  "RISK-1": "Max projected drawdown ≤ 20%",
  "RISK-2": "No single asset > 60% of portfolio",
  "RISK-3": "Per-leg slippage ≤ 75 bps",
  "FX-1": "EURC exposure in [0%, 40%] (Pro+)",
  "USYC-1": "USYC ≥ 10% when AUM ≥ $50k (Business)",
};

export interface ConstitutionClauseBadgeProps {
  clauseId: string;
  /** When true, render in veto/risk tone. When false, render as a positive
   * clean signal in the agent/cyan tone. Default true. */
  violated?: boolean;
  /** Optional explicit summary override (e.g. when the backend returns a
   * fuller description than the static fallback). */
  summary?: string;
}

/**
 * Small pill that surfaces one constitution clause ID next to the critic
 * verdict on the approval modal. Hover reveals the clause summary so the
 * user can see exactly which rule the strategist tripped — the auditable
 * rulebook behind the veto.
 */
export function ConstitutionClauseBadge({
  clauseId,
  violated = true,
  summary,
}: ConstitutionClauseBadgeProps) {
  const title = summary ?? CLAUSE_SUMMARIES[clauseId] ?? clauseId;
  return (
    <BrutalPill
      tone={violated ? "risk" : "agent"}
      title={title}
      aria-label={`Constitution clause ${clauseId}: ${title}`}
    >
      {clauseId}
    </BrutalPill>
  );
}
