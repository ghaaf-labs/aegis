"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { Bot, Pause, Play, Sparkles, Target } from "lucide-react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { agentApi, userAgentApi } from "@/lib/api";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";

export default function AgentStudioPage() {
  const portfolio = useActivePortfolio();
  const pausedAt = usePortfolioStore((s) => s.agentPausedAt);
  const setPausedAt = usePortfolioStore((s) => s.setAgentPausedAt);
  const addDecision = usePortfolioStore((s) => s.addDecision);
  const [busy, setBusy] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    userAgentApi
      .status()
      .then((status) => setPausedAt(status.pausedAt))
      .catch(() => {});
  }, [setPausedAt]);

  const toggleAgent = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const next = pausedAt
        ? await userAgentApi.resume()
        : await userAgentApi.pause();
      setPausedAt(next.pausedAt);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Agent toggle failed");
    } finally {
      setBusy(false);
    }
  }, [pausedAt, setPausedAt]);

  const runAnalysis = useCallback(async () => {
    if (!portfolio) return;
    setAnalyzing(true);
    setError(null);
    try {
      const decision = await agentApi.analyze(portfolio.id);
      addDecision(decision);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Analysis failed");
    } finally {
      setAnalyzing(false);
    }
  }, [addDecision, portfolio]);

  const paused = pausedAt !== null;

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Manual controls
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <Bot className="h-5 w-5 text-accent-agent" />
          Agent Studio
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          Run a manual analysis, pause scheduled triggers, and jump to the
          target-plan surfaces the agent reads before proposing a move.
        </p>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">Agent state</span>
            <BrutalPill tone={paused ? "neutral" : "agent"}>
              {paused ? "Paused" : "Active"}
            </BrutalPill>
          </BrutalCardHeader>
          <BrutalCardBody className="space-y-4">
            <p className="text-sm leading-relaxed text-text-lo">
              Pausing stops scheduled drift, regime, peg, and heartbeat
              triggers. Manual analysis and review flows remain available.
            </p>
            <BrutalButton
              type="button"
              variant={paused ? "agent" : "danger"}
              disabled={busy}
              onClick={() => void toggleAgent()}
            >
              {paused ? (
                <Play className="h-4 w-4" />
              ) : (
                <Pause className="h-4 w-4" />
              )}
              {busy ? "Working..." : paused ? "Resume agent" : "Pause agent"}
            </BrutalButton>
          </BrutalCardBody>
        </BrutalCard>

        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">
              Manual analysis
            </span>
            <BrutalPill tone="agent">OpenRouter</BrutalPill>
          </BrutalCardHeader>
          <BrutalCardBody className="space-y-4">
            <p className="text-sm leading-relaxed text-text-lo">
              Runs the strategist + critic loop against the current target,
              wallet cash, market snapshot, and recent decisions. It does not
              execute trades.
            </p>
            <BrutalButton
              type="button"
              variant="agent"
              disabled={!portfolio || analyzing}
              onClick={() => void runAnalysis()}
            >
              <Sparkles className="h-4 w-4" />
              {analyzing ? "Analyzing..." : "Run analysis"}
            </BrutalButton>
            {!portfolio && (
              <p className="text-xs font-mono text-warn">
                Create a portfolio before running analysis.
              </p>
            )}
          </BrutalCardBody>
        </BrutalCard>
      </div>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">
            Inputs the agent reads
          </span>
        </BrutalCardHeader>
        <BrutalCardBody className="grid gap-3 md:grid-cols-3">
          <StudioLink
            href="/portfolio"
            title="Target weights"
            body="Review current target allocation and drift."
          />
          <StudioLink
            href="/wallets"
            title="Wallet cash"
            body="Confirm idle USDC and EURC before deployment."
          />
          <StudioLink
            href="/settings/peg"
            title="Peg defense"
            body="Tune stablecoin thresholds and alert rules."
          />
        </BrutalCardBody>
      </BrutalCard>

      {error && (
        <p className="border border-risk/40 bg-risk/5 px-3 py-2 text-xs font-mono text-risk">
          {error}
        </p>
      )}
    </div>
  );
}

function StudioLink({
  href,
  title,
  body,
}: {
  href: string;
  title: string;
  body: string;
}) {
  return (
    <Link
      href={href}
      className="group border border-border-default bg-bg px-4 py-3 hover:border-accent-agent/50 hover:bg-accent-agent/5"
    >
      <div className="flex items-center gap-2 text-sm font-mono font-semibold text-text-hi">
        <Target className="h-4 w-4 text-accent-agent" />
        {title}
      </div>
      <p className="mt-2 text-xs font-mono leading-relaxed text-text-lo">
        {body}
      </p>
    </Link>
  );
}
