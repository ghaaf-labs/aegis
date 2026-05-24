"use client";

import { useMemo, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { Brain } from "lucide-react";
import type { AgentDecision } from "@/types";
import { DecisionRow } from "./decision-log-row";
import {
  groupRepeatedDecisions,
  hasTradeLegs,
  isAuditDecision,
  isCriticBlocked,
  isLegacyLocalDecision,
  isOutdatedForCurrentState,
  type CurrentDecisionState,
  type DecisionView,
} from "./decision-log-utils";

export function DecisionList({
  decisions,
  currentState,
}: {
  decisions: AgentDecision[];
  currentState: CurrentDecisionState;
}) {
  const [view, setView] = useState<DecisionView>("current");
  const currentDecisions = useMemo(
    () => decisions.filter((d) => !isAuditDecision(d, currentState)),
    [decisions, currentState],
  );
  const actionableCurrentDecisions = useMemo(
    () => currentDecisions.filter(hasTradeLegs),
    [currentDecisions],
  );
  const quietCurrentCount =
    currentDecisions.length - actionableCurrentDecisions.length;
  const visibleDecisions =
    view === "current" ? actionableCurrentDecisions : decisions;
  const groups = useMemo(
    () => groupRepeatedDecisions(visibleDecisions),
    [visibleDecisions],
  );
  const counts = decisionCounts({
    actionableCurrentDecisions,
    currentDecisions,
    decisions,
    quietCurrentCount,
    currentState,
  });

  if (decisions.length === 0) {
    return <EmptyDecisionState />;
  }

  return (
    <div className="min-h-0 flex-1 overflow-y-auto scrollbar-thin">
      <DecisionListTabs counts={counts} setView={setView} view={view} />
      <DecisionListNotices counts={counts} view={view} />
      <GroupedDecisionRows
        currentState={currentState}
        groups={groups}
        quietCurrentCount={quietCurrentCount}
        view={view}
      />
    </div>
  );
}

type DecisionCounts = {
  actionableCurrent: number;
  audit: number;
  blocked: number;
  legacy: number;
  quietCurrent: number;
  stale: number;
  total: number;
};

function decisionCounts({
  actionableCurrentDecisions,
  currentDecisions,
  currentState,
  decisions,
  quietCurrentCount,
}: {
  actionableCurrentDecisions: AgentDecision[];
  currentDecisions: AgentDecision[];
  currentState: CurrentDecisionState;
  decisions: AgentDecision[];
  quietCurrentCount: number;
}): DecisionCounts {
  return {
    actionableCurrent: actionableCurrentDecisions.length,
    audit: decisions.length - currentDecisions.length,
    blocked: decisions.filter(isCriticBlocked).length,
    legacy: decisions.filter(isLegacyLocalDecision).length,
    quietCurrent: quietCurrentCount,
    stale: decisions.filter((d) => isOutdatedForCurrentState(d, currentState))
      .length,
    total: decisions.length,
  };
}

function EmptyDecisionState() {
  return (
    <div className="flex min-h-[280px] flex-1 flex-col items-center justify-center px-6 py-12 text-center">
      <Brain className="w-6 h-6 text-accent-agent/30 mb-3" />
      <p className="text-xs font-mono text-text-mut">
        No decisions yet. The agent will reason here when triggered by drift, a
        regime flip, or your manual request.
      </p>
    </div>
  );
}

function DecisionListTabs({
  counts,
  setView,
  view,
}: {
  counts: DecisionCounts;
  setView: (view: DecisionView) => void;
  view: DecisionView;
}) {
  return (
    <div className="border-b border-border-default px-5 py-3">
      <div
        className="grid grid-cols-2 border border-border-default bg-bg p-1 text-[10px] font-mono"
        role="tablist"
        aria-label="Decision log view"
      >
        <DecisionViewButton
          active={view === "current"}
          onClick={() => setView("current")}
        >
          Current{" "}
          <span className="opacity-60">({counts.actionableCurrent})</span>
        </DecisionViewButton>
        <DecisionViewButton
          active={view === "audit"}
          onClick={() => setView("audit")}
        >
          History <span className="opacity-60">({counts.total})</span>
        </DecisionViewButton>
      </div>
      {view === "current" && counts.audit > 0 && (
        <p className="mt-2 text-[10px] font-mono leading-relaxed text-text-mut">
          {counts.audit} older or stale{" "}
          {counts.audit === 1 ? "row is" : "rows are"} in History.
        </p>
      )}
    </div>
  );
}

function DecisionListNotices({
  counts,
  view,
}: {
  counts: DecisionCounts;
  view: DecisionView;
}) {
  if (view === "current" && counts.actionableCurrent === 0) {
    return <NoCurrentPlanNotice quietCurrentCount={counts.quietCurrent} />;
  }
  if (
    view === "audit" &&
    (counts.blocked > 0 || counts.legacy > 0 || counts.stale > 0)
  ) {
    return <AuditSummary counts={counts} />;
  }
  return null;
}

function NoCurrentPlanNotice({
  quietCurrentCount,
}: {
  quietCurrentCount: number;
}) {
  return (
    <div className="px-5 py-8 text-center">
      <p className="text-xs font-mono text-text-lo">
        No current plan. Older and rejected proposals are available in History.
      </p>
      {quietCurrentCount > 0 && (
        <p className="mt-2 text-[10px] font-mono text-text-mut">
          {quietCurrentCount} monitor{" "}
          {quietCurrentCount === 1 ? "check reports" : "checks report"} no
          movement needed.
        </p>
      )}
    </div>
  );
}

function AuditSummary({ counts }: { counts: DecisionCounts }) {
  return (
    <div className="border-b border-border-default bg-bg px-5 py-3 text-[10px] font-mono leading-relaxed text-text-lo">
      {counts.stale > 0 && (
        <span className="text-warn">
          {counts.stale} cash-mismatch{" "}
          {counts.stale === 1 ? "proposal needs" : "proposals need"} a fresh
          plan
        </span>
      )}
      {counts.stale > 0 && (counts.blocked > 0 || counts.legacy > 0) && (
        <span className="text-text-mut"> - </span>
      )}
      {counts.blocked > 0 && (
        <span className="text-risk">
          {counts.blocked} critic-rejected{" "}
          {counts.blocked === 1 ? "proposal" : "proposals"}
        </span>
      )}
      {counts.blocked > 0 && counts.legacy > 0 && (
        <span className="text-text-mut"> - </span>
      )}
      {counts.legacy > 0 && (
        <span>
          {counts.legacy} old local{" "}
          {counts.legacy === 1 ? "decision" : "decisions"} kept for audit
        </span>
      )}
    </div>
  );
}

function GroupedDecisionRows({
  currentState,
  groups,
  quietCurrentCount,
  view,
}: {
  currentState: CurrentDecisionState;
  groups: Array<{ head: AgentDecision; repeats: number }>;
  quietCurrentCount: number;
  view: DecisionView;
}) {
  return (
    <>
      <AnimatePresence initial={false}>
        {groups.map((g, i) => (
          <div key={g.head.id}>
            <DecisionRow
              decision={g.head}
              index={i}
              currentState={currentState}
              view={view}
            />
            {g.repeats > 0 && <RepeatedDecisionFooter repeats={g.repeats} />}
          </div>
        ))}
      </AnimatePresence>
      {view === "current" && quietCurrentCount > 0 && groups.length > 0 && (
        <MonitorChecksFooter quietCurrentCount={quietCurrentCount} />
      )}
    </>
  );
}

function RepeatedDecisionFooter({ repeats }: { repeats: number }) {
  return (
    <div className="border-b border-border-default px-5 py-2 font-mono text-[10px] italic text-text-mut">
      + {repeats} earlier {repeats === 1 ? "decision" : "decisions"} with the
      same recommendation
    </div>
  );
}

function MonitorChecksFooter({
  quietCurrentCount,
}: {
  quietCurrentCount: number;
}) {
  return (
    <div className="border-t border-border-default px-5 py-3 text-[10px] font-mono text-text-mut">
      + {quietCurrentCount} monitor{" "}
      {quietCurrentCount === 1 ? "check reports" : "checks report"} no movement
      needed.
    </div>
  );
}

function DecisionViewButton({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={
        "min-h-9 border px-2 text-center transition-colors " +
        (active
          ? "border-border-default bg-raised text-text-hi"
          : "border-transparent text-text-lo hover:border-border-default hover:bg-raised hover:text-text-hi")
      }
    >
      {children}
    </button>
  );
}
