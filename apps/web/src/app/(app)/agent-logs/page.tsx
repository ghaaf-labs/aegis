"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { AlertTriangle, ArrowRight, Brain, SquareTerminal } from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
  ModelBadge,
} from "@aegis/ui";
import { agentApi } from "@/lib/api";
import { timeAgo } from "@/lib/utils";
import type { AgentDecision } from "@/types";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";

export default function AgentLogsPage() {
  const portfolio = useActivePortfolio();
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const storeDecisions = usePortfolioStore((s) => s.decisions);
  const setDecisions = usePortfolioStore((s) => s.setDecisions);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!portfolio) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    agentApi
      .decisions(portfolio.id)
      .then((rows) => {
        if (!cancelled) setDecisions(rows);
      })
      .catch((e) => {
        if (!cancelled)
          setError(
            e instanceof Error ? e.message : "Failed to load agent logs",
          );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [portfolio, setDecisions]);

  const decisions = storeDecisions.filter(
    (decision) => !portfolio || decision.portfolioId === portfolio.id,
  );
  const currentState = {
    investedUsd: portfolio?.totalValueUsd ?? 0,
    idleUsdc: unifiedUsdc,
  };
  const auditCount = decisions.filter((decision) =>
    isAuditOnly(decision, currentState),
  ).length;

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Decision history
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <SquareTerminal className="h-5 w-5 text-accent-agent" />
          Agent Logs
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          See what the agent recommended, how confident it was, and whether that
          recommendation still matches your current account.
        </p>
        {decisions.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-2">
            <BrutalPill tone="agent">
              {decisions.length - auditCount} current
            </BrutalPill>
            {auditCount > 0 && (
              <BrutalPill tone="warn">{auditCount} historical</BrutalPill>
            )}
          </div>
        )}
      </div>

      {error && (
        <p className="border border-risk/40 bg-risk/5 px-3 py-2 text-xs font-mono text-risk">
          {error}
        </p>
      )}

      <div className="space-y-3">
        {loading && decisions.length === 0 ? (
          <BrutalCard>
            <BrutalCardBody>
              <p className="text-xs font-mono text-text-lo">
                Loading agent logs...
              </p>
            </BrutalCardBody>
          </BrutalCard>
        ) : decisions.length === 0 ? (
          <BrutalCard>
            <BrutalCardBody>
              <p className="text-sm font-mono font-semibold text-text-hi">
                No agent decisions yet
              </p>
              <p className="mt-1 text-xs font-mono text-text-lo">
                Run a dashboard review or open Agent Studio to ask for a fresh
                recommendation.
              </p>
              <Link
                href="/agent-studio"
                className="mt-3 inline-flex items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent hover:border-accent-agent"
              >
                Open Agent Studio
                <ArrowRight className="h-3 w-3" />
              </Link>
            </BrutalCardBody>
          </BrutalCard>
        ) : (
          decisions.map((decision) => (
            <DecisionRow
              key={decision.id}
              decision={decision}
              currentState={currentState}
            />
          ))
        )}
      </div>
    </div>
  );
}

interface DecisionState {
  investedUsd: number;
  idleUsdc: number;
}

function DecisionRow({
  decision,
  currentState,
}: {
  decision: AgentDecision;
  currentState: DecisionState;
}) {
  const confidence = Math.round(decision.confidence * 100);
  const audit = auditReason(decision, currentState);
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <Brain className="h-4 w-4 text-accent-agent" />
          <span className="text-sm font-mono text-text-hi">
            {decision.triggeredBy.replaceAll("_", " ")}
          </span>
          {decision.modelSlug && <ModelBadge model={decision.modelSlug} />}
          {decision.regime && (
            <BrutalPill tone="agent">
              {decision.regime.replace("_", " ")}
            </BrutalPill>
          )}
          {audit && (
            <BrutalPill tone={audit.tone}>
              <AlertTriangle className="h-3 w-3" />
              Historical
            </BrutalPill>
          )}
        </div>
        <span className="text-[11px] font-mono text-text-lo">
          {timeAgo(decision.createdAt)}
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-3">
        {audit && (
          <p className="border border-warn/40 bg-warn/5 px-3 py-2 text-xs font-mono leading-relaxed text-warn">
            {audit.message}
          </p>
        )}
        <p className="text-sm leading-relaxed text-text-default">
          {decision.recommendation.summary || decision.reasoning}
        </p>
        <div>
          <div className="mb-1 flex items-center justify-between text-[11px] font-mono text-text-lo">
            <span>Confidence</span>
            <span>{confidence}%</span>
          </div>
          <div className="h-1.5 border border-border-default bg-bg">
            <div
              className="h-full bg-accent-agent"
              style={{ width: `${confidence}%` }}
            />
          </div>
        </div>
        {decision.criticVerdict && (
          <p className="border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 text-xs font-mono leading-relaxed text-text-lo">
            <span className="text-accent-agent">
              Safety check {decision.criticVerdict.verdict ?? "result"}:
            </span>{" "}
            {decision.criticVerdict.notes}
          </p>
        )}
      </BrutalCardBody>
    </BrutalCard>
  );
}

function isAuditOnly(decision: AgentDecision, currentState: DecisionState) {
  return auditReason(decision, currentState) !== null;
}

function auditReason(
  decision: AgentDecision,
  currentState: DecisionState,
): { tone: "warn" | "risk"; message: string } | null {
  const blocked =
    decision.criticVerdict?.demandsRevision === true ||
    decision.criticVerdict?.verdict === "revised" ||
    decision.criticVerdict?.verdict === "veto";
  if (blocked) {
    return {
      tone: "risk",
      message:
        "The critic did not approve this recommendation. Keep it as audit evidence and build a fresh review before acting.",
    };
  }

  const snapshot = decision.snapshot ?? {};
  if (snapshot.planner === "deterministic") {
    const investedValueUsd = deterministicSnapshotInvestedUsd(snapshot);
    const idleUsdc = Number(snapshot.idleUsdc);
    const portfolioMismatch =
      Number.isFinite(investedValueUsd) &&
      Math.abs(investedValueUsd - currentState.investedUsd) > 0.5;
    const idleMismatch =
      Number.isFinite(idleUsdc) &&
      Math.abs(idleUsdc - currentState.idleUsdc) > 0.5;
    if (portfolioMismatch || idleMismatch) {
      return {
        tone: "warn",
        message: `Historical input snapshot. It was built from ${formatUsd(
          Number.isFinite(investedValueUsd) ? investedValueUsd : 0,
        )} invested and ${formatUsd(
          Number.isFinite(idleUsdc) ? idleUsdc : 0,
        )} idle USDC; current state is ${formatUsd(
          currentState.investedUsd,
        )} invested and ${formatUsd(currentState.idleUsdc)} idle USDC.`,
      };
    }
  }

  const text =
    `${decision.recommendation.summary ?? ""} ${decision.reasoning}`.toLowerCase();
  const saysEmptyOrNeedsFunds =
    /\b(empty|zero market value|deposit|fund the account|no confirmed positions)\b/.test(
      text,
    );
  if (
    saysEmptyOrNeedsFunds &&
    currentState.investedUsd + currentState.idleUsdc > 5
  ) {
    return {
      tone: "warn",
      message:
        "Historical context mismatch. This row says the account was empty or needed funding, but the current wallet/portfolio now has value.",
    };
  }

  return null;
}

function deterministicSnapshotInvestedUsd(snapshot: Record<string, unknown>) {
  const explicit = Number(snapshot.investedValueUsd);
  if (Number.isFinite(explicit)) return explicit;
  const planValue = Number(snapshot.planValueUsd ?? snapshot.portfolioValueUsd);
  const idleUsdc = Number(snapshot.idleUsdc);
  if (Number.isFinite(planValue) && Number.isFinite(idleUsdc)) {
    return Math.max(0, planValue - idleUsdc);
  }
  return Number(snapshot.portfolioValueUsd);
}

function formatUsd(value: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  }).format(value);
}
