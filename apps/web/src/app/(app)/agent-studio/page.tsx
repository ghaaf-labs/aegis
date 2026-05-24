"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  Bot,
  CheckCircle2,
  CircleAlert,
  Pause,
  Play,
  Sparkles,
  Target,
  Wallet,
} from "lucide-react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { agentApi, userAgentApi } from "@/lib/api";
import { pollDecisionReady } from "@/lib/decision-poll";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { totalWalletCashUsd } from "@/lib/cash-model";

const RECOMMENDATION_TIMEOUT_MS = 45_000;

export default function AgentStudioPage() {
  const portfolio = useActivePortfolio();
  const pausedAt = usePortfolioStore((s) => s.agentPausedAt);
  const setPausedAt = usePortfolioStore((s) => s.setAgentPausedAt);
  const addDecision = usePortfolioStore((s) => s.addDecision);
  const wallet = usePortfolioStore((s) => s.wallet);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const marketSnapshot = usePortfolioStore((s) => s.marketSnapshot);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const [busy, setBusy] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const idleCashUsd = totalWalletCashUsd(
    unifiedUsdc,
    unifiedEurc,
    marketSnapshot,
  );
  const investedUsd = derivePortfolioPositionMetrics(
    portfolio,
    marketSnapshot,
  ).investedUsd;
  const hasCapital = idleCashUsd > 0.5 || investedUsd > 0.5;
  const analysisBlocked =
    !portfolio || !wallet || gatewayBalanceStatus !== "ready" || !hasCapital;
  const analysisBlock = manualAnalysisBlockCopy(
    !!portfolio,
    !!wallet,
    gatewayBalanceStatus,
    gatewayBalanceError,
    hasCapital,
  );

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
    if (analysisBlocked) {
      setError(analysisBlock.copy);
      return;
    }
    setAnalyzing(true);
    setError(null);
    setNotice(null);
    try {
      // Async: analyze enqueues a job and returns a `queued` placeholder; poll
      // until the worker finishes (the SSE `agent.decision` also lands it live).
      const queued = await agentApi.analyze(
        portfolio.id,
        RECOMMENDATION_TIMEOUT_MS,
      );
      const decision = await pollDecisionReady(queued.id);
      addDecision(decision);
      setNotice("Recommendation ready. Open Dashboard to review it.");
    } catch (e) {
      setError(recommendationErrorCopy(e));
    } finally {
      setAnalyzing(false);
    }
  }, [addDecision, analysisBlock.copy, analysisBlocked, portfolio]);

  const paused = pausedAt !== null;

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <section className="rounded-sharp border-brutal border-border-default bg-surface p-4 md:p-5">
        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(280px,360px)] lg:items-end">
          <div>
            <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
              Agent controls
            </p>
            <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
              <Bot className="h-5 w-5 text-accent-agent" />
              Agent Studio
            </h1>
            <p className="mt-2 max-w-2xl text-sm leading-relaxed text-text-lo">
              Ask for one recommendation, pause background checks, and see what
              setup is still needed. Nothing moves without your approval.
            </p>
          </div>
          <div className="grid gap-2 font-mono text-[11px]">
            <ReadinessRow
              ready={!!portfolio}
              label="Portfolio"
              value={portfolio ? portfolio.name : "Create target first"}
            />
            <ReadinessRow
              ready={!!wallet}
              label="Wallet"
              value={wallet ? "Connected" : "Not ready"}
            />
            <ReadinessRow
              ready={gatewayBalanceStatus === "ready"}
              label="Balance"
              value={
                gatewayBalanceStatus === "ready"
                  ? hasCapital
                    ? "Capital available"
                    : "No capital yet"
                  : gatewayBalanceStatus === "error"
                    ? "Check failed"
                    : "Checking"
              }
              warn={gatewayBalanceStatus === "error" || !hasCapital}
            />
          </div>
        </div>
      </section>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]">
        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">Agent state</span>
            <BrutalPill tone={paused ? "neutral" : "agent"}>
              {paused ? "Paused" : "Active"}
            </BrutalPill>
          </BrutalCardHeader>
          <BrutalCardBody className="space-y-4">
            <p className="text-sm leading-relaxed text-text-lo">
              Pausing only stops background checks. Existing reviews and manual
              recommendations stay available.
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
              Fresh recommendation
            </span>
            <BrutalPill tone="agent">Review only</BrutalPill>
          </BrutalCardHeader>
          <BrutalCardBody className="space-y-4">
            <p className="text-sm leading-relaxed text-text-lo">
              Aegis reads your target mix, wallet cash, market data, and recent
              decisions, then creates a review plan. It does not execute from
              this screen.
            </p>
            {analysisBlocked && (
              <div className="border border-warn/40 bg-warn/5 px-3 py-2 font-mono">
                <div className="flex items-start gap-2">
                  <CircleAlert className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
                  <div>
                    <p className="text-[10px] uppercase tracking-widest text-warn">
                      Recommendation needs setup
                    </p>
                    <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                      {analysisBlock.copy}
                    </p>
                  </div>
                </div>
                {analysisBlock.href && (
                  <Link
                    href={analysisBlock.href}
                    className="mt-3 inline-flex min-h-8 items-center justify-center border border-warn/40 px-2 py-1 text-[11px] font-semibold text-warn hover:bg-warn/10"
                  >
                    {analysisBlock.cta}
                  </Link>
                )}
              </div>
            )}
            <BrutalButton
              type="button"
              variant="agent"
              disabled={analysisBlocked || analyzing}
              onClick={() => void runAnalysis()}
            >
              <Sparkles className="h-4 w-4" />
              {analyzing
                ? "Preparing recommendation..."
                : analysisBlocked
                  ? "Set up first"
                  : "Get recommendation"}
            </BrutalButton>
            {notice && (
              <Link
                href="/dashboard"
                className="block border border-accent-agent/40 bg-accent-agent/5 px-3 py-2 text-xs font-mono text-accent-agent hover:bg-accent-agent/10"
              >
                {notice}
              </Link>
            )}
          </BrutalCardBody>
        </BrutalCard>
      </div>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">Before asking</span>
        </BrutalCardHeader>
        <BrutalCardBody className="grid gap-3 md:grid-cols-3">
          <StudioLink
            href="/portfolio"
            title="Target weights"
            body="Check the mix Aegis will compare against."
          />
          <StudioLink
            href="/wallets"
            title="Wallet cash"
            body="Confirm available cash before planning a move."
          />
          <StudioLink
            href="/settings/peg"
            title="Peg defense"
            body="Set stablecoin guardrails and alert rules."
          />
        </BrutalCardBody>
      </BrutalCard>

      {error && (
        <p
          role="alert"
          className="border border-risk/40 bg-risk/5 px-3 py-2 text-xs font-mono text-risk"
        >
          {error}
        </p>
      )}
    </div>
  );
}

function ReadinessRow({
  ready,
  warn = false,
  label,
  value,
}: {
  ready: boolean;
  warn?: boolean;
  label: string;
  value: string;
}) {
  const toneClass = ready
    ? "border-accent-pnl/35 bg-accent-pnl/5 text-accent-pnl"
    : warn
      ? "border-warn/40 bg-warn/5 text-warn"
      : "border-border-default bg-bg text-text-lo";
  const Icon = ready ? CheckCircle2 : warn ? CircleAlert : Wallet;

  return (
    <div
      className={`grid min-h-10 grid-cols-[auto_minmax(0,1fr)] items-center gap-2 rounded-sharp border px-3 py-2 ${toneClass}`}
    >
      <Icon className="h-3.5 w-3.5" />
      <div className="min-w-0">
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
        <p className="truncate text-xs font-semibold text-text-hi">{value}</p>
      </div>
    </div>
  );
}

function recommendationErrorCopy(error: unknown) {
  const message = error instanceof Error ? error.message : "";
  if (message.includes("Request timed out")) {
    return "Aegis took too long to prepare a recommendation. Try again.";
  }
  if (message.startsWith("401:")) {
    return "Your session expired. Sign in again to continue.";
  }
  return message || "Aegis could not prepare a recommendation. Try again.";
}

function manualAnalysisBlockCopy(
  hasPortfolio: boolean,
  hasWallet: boolean,
  gatewayBalanceStatus: "idle" | "loading" | "ready" | "error",
  gatewayBalanceError: string | null,
  hasCapital: boolean,
) {
  if (!hasPortfolio) {
    return {
      copy: "Create a portfolio target before asking the agent for allocation advice.",
      href: "/onboarding",
      cta: "Create portfolio",
    };
  }
  if (!hasWallet) {
    return {
      copy: "Finish account setup first. The agent needs a ready wallet before it can use wallet cash in a recommendation.",
      href: "/wallets",
      cta: "Check account setup",
    };
  }
  if (gatewayBalanceStatus === "error") {
    return {
      copy:
        gatewayBalanceError ??
        "The balance check did not return wallet cash. Recommendations wait so unknown cash is not treated as zero.",
      href: "/wallets",
      cta: "Open wallet status",
    };
  }
  if (gatewayBalanceStatus === "ready" && !hasCapital) {
    return {
      copy: "Add wallet cash or hold an invested position before asking for a recommendation.",
      href: "/wallets",
      cta: "Add test USDC",
    };
  }
  return {
    copy: "Aegis is still checking wallet cash. Recommendations unlock after balances are confirmed.",
    href: "/wallets",
    cta: "Check wallet status",
  };
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
