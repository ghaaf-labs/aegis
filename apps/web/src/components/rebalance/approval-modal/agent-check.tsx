import { useState } from "react";
import { ModelBadge } from "@aegis/ui";
import type { AgentDecision } from "@/types";
import { ConstitutionClauseBadge } from "@/components/agent/constitution-clause-badge";
import { displayReasoning, pickHeadlineConfidence } from "./helpers";

export function AgentCheck({ decision }: { decision: AgentDecision }) {
  const [counterfactualOpen, setCounterfactualOpen] = useState(false);

  const headline = pickHeadlineConfidence(decision);
  const headlinePct = Math.round(headline * 100);
  const isCalibrated = typeof decision.calibratedConfidence === "number";
  const raw = decision.rawConfidence ?? decision.confidence ?? 0;
  const rawPct = Math.round(raw * 100);
  const clauseIds = decision.criticVerdict?.clauseIds ?? [];
  const criticWarning =
    decision.criticVerdict?.demandsRevision === true ||
    decision.criticVerdict?.verdict === "veto" ||
    decision.criticVerdict?.verdict === "revised";

  return (
    <details className="mb-3 border border-white/10 bg-black/30 p-3 font-mono text-[11px]">
      <summary className="cursor-pointer list-none">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-accent-agent uppercase tracking-wider">
            Agent check
          </span>
          <span className="border border-accent-agent/30 bg-accent-agent/10 px-2 py-0.5 text-accent-agent">
            {headlinePct}% confidence
          </span>
          <span
            className={
              criticWarning
                ? "border border-warn/30 bg-warn/10 px-2 py-0.5 text-warn"
                : "border border-accent-pnl/30 bg-accent-pnl/10 px-2 py-0.5 text-accent-pnl"
            }
          >
            {criticWarning ? "Critic warning" : "Critic passed"}
          </span>
          <span className="ml-auto text-text-mut">details</span>
        </div>
      </summary>
      <div className="mt-3 space-y-2 border-t border-white/5 pt-3">
        <div className="flex flex-wrap items-center gap-2">
          {decision.modelSlug && <ModelBadge model={decision.modelSlug} />}
          {decision.regime && (
            <span className="px-1.5 py-0.5 bg-violet-500/10 border border-violet-500/30 text-violet-200">
              regime: {decision.regime}
            </span>
          )}
          {isCalibrated && <span className="text-text-mut">raw {rawPct}%</span>}
        </div>

        {(() => {
          const reasoning = displayReasoning(decision);
          return reasoning ? (
            <p className="text-text-lo leading-relaxed">{reasoning}</p>
          ) : null;
        })()}

        {clauseIds.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {clauseIds.map((id) => (
              <ConstitutionClauseBadge key={id} clauseId={id} violated />
            ))}
          </div>
        )}
        {decision.criticVerdict &&
          clauseIds.length === 0 &&
          decision.criticVerdict.verdict !== "veto" && (
            <ConstitutionClauseBadge
              clauseId="Constitution clean"
              violated={false}
              summary="No hard constraints violated. Critic ran free-form review only."
            />
          )}
        {decision.criticVerdict && (
          <p className="text-warn/90">Critic: {decision.criticVerdict.notes}</p>
        )}
        {decision.counterfactual && (
          <div>
            <button
              type="button"
              onClick={() => setCounterfactualOpen((v) => !v)}
              className="text-[10px] uppercase tracking-wider text-accent-agent hover:text-accent-agent/70"
            >
              {counterfactualOpen ? "Hide" : "Show"} risk note
            </button>
            {counterfactualOpen && (
              <p className="mt-1.5 text-[11px] text-accent-agent/70 bg-cyan-500/5 border border-cyan-500/20 px-2 py-1.5">
                {decision.counterfactual}
              </p>
            )}
          </div>
        )}
      </div>
    </details>
  );
}
