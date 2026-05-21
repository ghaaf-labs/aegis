"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { ArrowRight, Brain, SquareTerminal } from "lucide-react";
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

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Reasoning archive
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <SquareTerminal className="h-5 w-5 text-accent-agent" />
          Agent Logs
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          Every strategist note, critic verdict, model slug, and confidence
          value in one scan-friendly log.
        </p>
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
                No agent logs yet
              </p>
              <p className="mt-1 text-xs font-mono text-text-lo">
                Run a dashboard review or open Agent Studio to create a manual
                analysis.
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
            <DecisionRow key={decision.id} decision={decision} />
          ))
        )}
      </div>
    </div>
  );
}

function DecisionRow({ decision }: { decision: AgentDecision }) {
  const confidence = Math.round(decision.confidence * 100);
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
        </div>
        <span className="text-[11px] font-mono text-text-lo">
          {timeAgo(decision.createdAt)}
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-3">
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
              Critic {decision.criticVerdict.verdict ?? "verdict"}:
            </span>{" "}
            {decision.criticVerdict.notes}
          </p>
        )}
      </BrutalCardBody>
    </BrutalCard>
  );
}
