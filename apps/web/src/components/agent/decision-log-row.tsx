"use client";

import { motion } from "framer-motion";
import { BrutalBadge as Badge } from "@aegis/ui";
import { formatCurrency, timeAgo } from "@/lib/utils";
import type { AgentDecision } from "@/types";
import { EvidencePanel } from "./decision-log-evidence";
import { TradeTable } from "./decision-log-trades";
import {
  decisionExpectedCashNeeded,
  decisionHeadline,
  decisionSnapshotMismatch,
  decisionStatusCopy,
  decisionTotalTradeUsd,
  hasTradeLegs,
  isCriticBlocked,
  isLegacyLocalDecision,
  isOutdatedForCurrentState,
  knownRegime,
  knownTrigger,
  TRIGGER_LABELS,
  TRIGGER_VARIANTS,
  type CurrentDecisionState,
  type DecisionStatus,
  type DecisionView,
} from "./decision-log-utils";

export function DecisionRow({
  decision,
  index,
  currentState,
  view,
}: {
  decision: AgentDecision;
  index: number;
  currentState: CurrentDecisionState;
  view: DecisionView;
}) {
  const trigger = knownTrigger(decision.triggeredBy);
  const triggerVariant = TRIGGER_VARIANTS[trigger] ?? "secondary";
  const triggerLabel = TRIGGER_LABELS[trigger] ?? trigger;
  const regime = knownRegime(decision.regime);
  const verdict = decision.criticVerdict;
  const blocked = isCriticBlocked(decision);
  const legacyLocal = isLegacyLocalDecision(decision);
  const outdated = isOutdatedForCurrentState(decision, currentState);
  const trades = decision.recommendation?.trades ?? [];
  const expectedCashNeeded = decisionExpectedCashNeeded(decision);
  const snapshotMismatch = decisionSnapshotMismatch(decision, currentState);
  const showAuditDetails = view === "audit";
  const totalUsd = decisionTotalTradeUsd(decision);
  const status = decisionStatusCopy({
    blocked,
    legacyLocal,
    outdated,
    showAuditDetails,
    tradesCount: trades.length,
  });

  return (
    <motion.div
      initial={{ opacity: 0, x: 8 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: Math.min(index * 0.04, 0.5) }}
      className={`border-b border-border-default px-5 py-4 last:border-0 transition-colors hover:bg-white/2 ${
        blocked || legacyLocal || outdated ? "bg-white/[0.015]" : ""
      }`}
    >
      <div className="grid gap-3 xl:grid-cols-[minmax(0,1.45fr)_minmax(280px,0.75fr)]">
        <RecommendationPanel
          blocked={blocked}
          currentState={currentState}
          decision={decision}
          expectedCashNeeded={expectedCashNeeded}
          legacyLocal={legacyLocal}
          outdated={outdated}
          showAuditDetails={showAuditDetails}
          snapshotMismatch={snapshotMismatch}
          status={status}
          totalUsd={totalUsd}
          trades={trades}
          triggerLabel={triggerLabel}
          triggerVariant={triggerVariant}
        />
        <EvidencePanel
          confidence={decision.confidence}
          modelSlug={decision.modelSlug}
          regime={regime}
          showAuditDetails={showAuditDetails}
          status={status}
          verdict={verdict}
          decision={decision}
        />
      </div>
    </motion.div>
  );
}

function RecommendationPanel({
  blocked,
  currentState,
  decision,
  expectedCashNeeded,
  legacyLocal,
  outdated,
  showAuditDetails,
  snapshotMismatch,
  status,
  totalUsd,
  trades,
  triggerLabel,
  triggerVariant,
}: {
  blocked: boolean;
  currentState: CurrentDecisionState;
  decision: AgentDecision;
  expectedCashNeeded: number;
  legacyLocal: boolean;
  outdated: boolean;
  showAuditDetails: boolean;
  snapshotMismatch: ReturnType<typeof decisionSnapshotMismatch>;
  status: DecisionStatus;
  totalUsd: number;
  trades: AgentDecision["recommendation"]["trades"];
  triggerLabel: string;
  triggerVariant: "warning" | "default" | "secondary" | "danger";
}) {
  const dimmed = blocked || legacyLocal || outdated;
  return (
    <section
      className="min-w-0 border border-border-default bg-bg p-3"
      aria-label="Decision recommendation"
    >
      <DecisionSummaryHeader
        createdAt={decision.createdAt}
        dimmed={dimmed}
        headline={recommendationHeadline(decision, showAuditDetails)}
        summary={recommendationSummary(decision, showAuditDetails, status)}
        totalUsd={totalUsd}
        triggerLabel={triggerLabel}
        triggerVariant={triggerVariant}
      />
      <AuditNotice
        blocked={blocked}
        currentState={currentState}
        expectedCashNeeded={expectedCashNeeded}
        legacyLocal={legacyLocal}
        outdated={outdated}
        showAuditDetails={showAuditDetails}
        snapshotMismatch={snapshotMismatch}
      />
      {trades.length > 0 ? (
        <TradeTable
          blocked={blocked || legacyLocal}
          decisionId={decision.id}
          trades={trades}
        />
      ) : (
        <div className="mt-3 border border-border-default bg-raised px-3 py-3 text-[11px] text-text-lo">
          No movement is proposed right now.
        </div>
      )}
    </section>
  );
}

function DecisionSummaryHeader({
  createdAt,
  dimmed,
  headline,
  summary,
  totalUsd,
  triggerLabel,
  triggerVariant,
}: {
  createdAt: string;
  dimmed: boolean;
  headline: string;
  summary: string;
  totalUsd: number;
  triggerLabel: string;
  triggerVariant: "warning" | "default" | "secondary" | "danger";
}) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border-default pb-3">
      <div className="min-w-0">
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <Badge variant={triggerVariant} className="px-1.5 py-0 text-[10px]">
            {triggerLabel}
          </Badge>
          <span className="font-mono text-[10px] text-text-mut">
            {timeAgo(createdAt)}
          </span>
        </div>
        <p
          className={`text-sm font-semibold leading-snug ${
            dimmed ? "text-text-lo" : "text-text-hi"
          }`}
        >
          {headline}
        </p>
        <p className="mt-1 text-[11px] leading-relaxed text-text-mut">
          {summary}
        </p>
      </div>
      <div className="shrink-0 text-right">
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          Proposed move
        </p>
        <p
          className={`mt-1 font-mono text-xl font-semibold tabular-nums ${
            totalUsd > 0 ? "text-accent-pnl" : "text-text-hi"
          }`}
        >
          {totalUsd > 0 ? formatCurrency(totalUsd) : "None"}
        </p>
      </div>
    </div>
  );
}

function recommendationHeadline(
  decision: AgentDecision,
  showAuditDetails: boolean,
) {
  if (!showAuditDetails) return decisionHeadline(decision);
  return (
    decision.recommendation?.summary ??
    ((decision.recommendation?.trades ?? []).length > 0
      ? "Decision needs review"
      : "No action proposed")
  );
}

function recommendationSummary(
  decision: AgentDecision,
  showAuditDetails: boolean,
  status: DecisionStatus,
) {
  if (!showAuditDetails) {
    return hasTradeLegs(decision)
      ? "No funds move until you approve."
      : status.body;
  }
  return decision.reasoning || "No reasoning was returned with this decision.";
}

function AuditNotice({
  blocked,
  currentState,
  expectedCashNeeded,
  legacyLocal,
  outdated,
  showAuditDetails,
  snapshotMismatch,
}: {
  blocked: boolean;
  currentState: CurrentDecisionState;
  expectedCashNeeded: number;
  legacyLocal: boolean;
  outdated: boolean;
  showAuditDetails: boolean;
  snapshotMismatch: ReturnType<typeof decisionSnapshotMismatch>;
}) {
  if (!showAuditDetails) return null;
  if (blocked) {
    return (
      <p className="mt-3 border border-risk/40 bg-risk/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-risk">
        Not executable. The critic rejected this proposal; build a fresh plan
        before approving any movement.
      </p>
    );
  }
  if (legacyLocal) {
    return (
      <p className="mt-3 border border-border-default bg-raised px-3 py-2 font-mono text-[11px] leading-relaxed text-text-mut">
        Historical test row only. It does not describe the current
        real-execution path.
      </p>
    );
  }
  if (!outdated) return null;
  const mismatch = snapshotMismatch;
  return (
    <p className="mt-3 border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-warn">
      {mismatch
        ? `Historical proposal. It was built from ${formatCurrency(mismatch.portfolioValueUsd)} positions and ${formatCurrency(mismatch.idleUsdc)} idle USDC; current state is ${formatCurrency(currentState.investedUsd)} positions and ${formatCurrency(currentState.idleUsdc)} idle USDC.`
        : `Historical proposal. It expects roughly ${formatCurrency(expectedCashNeeded)} deployable USDC, but current idle USDC is ${formatCurrency(currentState.idleUsdc)}.`}{" "}
      Build a fresh review plan before acting.
    </p>
  );
}
