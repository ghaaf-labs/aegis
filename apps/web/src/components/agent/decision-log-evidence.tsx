"use client";

import { Cpu, ShieldAlert, Zap } from "lucide-react";
import type { AgentDecision, CriticVerdict, MarketRegime } from "@/types";
import {
  isRevisionVerdict,
  REGIME_CLASS,
  REGIME_LABEL,
  safeConfidence,
  type DecisionStatus,
  type DecisionStatusTone,
} from "./decision-log-utils";

export function EvidencePanel({
  confidence,
  decision,
  modelSlug,
  regime,
  showAuditDetails,
  status,
  verdict,
}: {
  confidence: number;
  decision: AgentDecision;
  modelSlug?: string;
  regime: MarketRegime | null;
  showAuditDetails: boolean;
  status: DecisionStatus;
  verdict?: CriticVerdict;
}) {
  return (
    <aside
      className="grid content-start gap-2 border border-border-default bg-bg p-3"
      aria-label="Decision evidence"
    >
      <DecisionStatusBox status={status} />
      {modelSlug && <ModelSlugBox modelSlug={modelSlug} />}
      <div className="grid grid-cols-2 gap-2">
        {regime && (
          <MetricBox
            label="Regime"
            value={REGIME_LABEL[regime]}
            className={REGIME_CLASS[regime]}
          />
        )}
        <ConfidenceBox confidence={confidence} />
      </div>
      {verdict && (isRevisionVerdict(verdict) || verdict.notes) && (
        <CriticLine compact={!showAuditDetails} verdict={verdict} />
      )}
      {showAuditDetails && <TelemetryFooter decision={decision} />}
    </aside>
  );
}

function DecisionStatusBox({
  status,
}: {
  status: { body: string; label: string; tone: DecisionStatusTone };
}) {
  const toneClass = {
    agent: "border-l-accent-agent text-accent-agent",
    warn: "border-l-warn text-warn",
    risk: "border-l-risk text-risk",
    muted: "border-l-border-default text-text-lo",
  }[status.tone];
  return (
    <div
      className={`border border-l-[3px] border-border-default bg-bg px-3 py-2 ${toneClass}`}
    >
      <p className="font-mono text-[10px] uppercase tracking-widest">
        {status.label}
      </p>
      <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
        {status.body}
      </p>
    </div>
  );
}

function ModelSlugBox({ modelSlug }: { modelSlug: string }) {
  return (
    <div className="flex min-w-0 items-center gap-2 border border-border-default bg-bg px-3 py-2">
      <Cpu className="h-3.5 w-3.5 shrink-0 text-accent-agent" />
      <span
        className="truncate font-mono text-[11px] text-text-hi"
        title={modelSlug}
      >
        {modelSlug}
      </span>
    </div>
  );
}

function MetricBox({
  className,
  label,
  value,
}: {
  className?: string;
  label: string;
  value: string;
}) {
  return (
    <div
      className={`border px-3 py-2 ${
        className ?? "border-border-default bg-bg"
      }`}
    >
      <p className="font-mono text-[10px] uppercase tracking-widest opacity-65">
        {label}
      </p>
      <p className="mt-1 truncate font-mono text-[11px] font-semibold">
        {value}
      </p>
    </div>
  );
}

function ConfidenceBox({ confidence }: { confidence: number }) {
  const pct = Math.round(safeConfidence(confidence) * 100);
  return (
    <div className="border border-border-default bg-bg px-3 py-2">
      <div className="flex items-center justify-between gap-2">
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          Confidence
        </p>
        <span className="flex items-center gap-1 font-mono text-[11px] font-semibold tabular-nums text-accent-agent">
          <Zap className="h-3 w-3" />
          {pct}%
        </span>
      </div>
      <div
        className="mt-2 h-1.5 border border-border-default bg-bg"
        role="meter"
        aria-label="Strategist confidence"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct}
      >
        <div className="h-full bg-accent-agent" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function CriticLine({
  compact = false,
  verdict,
}: {
  compact?: boolean;
  verdict: CriticVerdict;
}) {
  const demandsRevision = isRevisionVerdict(verdict);
  const tone = demandsRevision
    ? "border-l-risk text-risk"
    : "border-l-accent-agent text-accent-agent";
  return (
    <div
      className={`mt-3 flex items-start gap-2 rounded-sharp border border-l-[3px] border-border-default bg-bg px-2 py-1.5 ${tone} text-[10px] leading-relaxed ${
        compact ? "max-h-14 overflow-hidden" : ""
      }`}
    >
      <ShieldAlert className="w-3 h-3 mt-0.5 shrink-0" />
      <span className="font-mono">
        <span className="opacity-70">
          Critic ({Math.round((verdict.confidence ?? 0) * 100)}%):
        </span>{" "}
        {demandsRevision ? "Revision requested" : "Approved"}
        {compact ? (
          verdict.notes ? (
            <span className="text-text-lo"> - {verdict.notes}</span>
          ) : null
        ) : (
          <>
            {" - "}
            <span className="opacity-90">{verdict.notes || "(no notes)"}</span>
          </>
        )}
      </span>
    </div>
  );
}

function TelemetryFooter({ decision }: { decision: AgentDecision }) {
  const hasTelemetry =
    decision.promptTokens != null ||
    decision.completionTokens != null ||
    decision.latencyMs != null;
  if (!hasTelemetry) return null;
  return (
    <div className="mt-2 flex items-center gap-3 text-[10px] font-mono text-text-mut">
      {decision.promptTokens != null && (
        <span title="Prompt tokens">in {decision.promptTokens}</span>
      )}
      {decision.completionTokens != null && (
        <span title="Completion tokens">out {decision.completionTokens}</span>
      )}
      {decision.latencyMs != null && (
        <span title="End-to-end latency">{decision.latencyMs}ms</span>
      )}
    </div>
  );
}
