"use client";

import {
  BarChart3,
  Brain,
  CircleDollarSign,
  PieChart,
  ShieldCheck,
} from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { formatCurrency, formatPercent, timeAgo } from "@/lib/utils";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import {
  deriveIdleCashUsd,
  derivePortfolioPositionMetrics,
} from "@/lib/portfolio-values";

export default function AnalyticsPage() {
  const portfolio = useActivePortfolio();
  const decisions = usePortfolioStore((s) => s.decisions);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const regime = usePortfolioStore((s) => s.regime);
  const avgConfidence =
    decisions.length > 0
      ? decisions.reduce((sum, d) => sum + d.confidence, 0) / decisions.length
      : 0;
  const hasMarketCap = (snapshot?.totalMarketCapUsd ?? 0) > 0;
  const hasBtcDominance = (snapshot?.btcDominance ?? 0) > 0;
  const positionMetrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const investedUsd = positionMetrics.investedUsd;
  const idleCashUsd = deriveIdleCashUsd(unifiedUsdc, unifiedEurc, snapshot);
  const netWorth = investedUsd + idleCashUsd;
  const hasConfirmedCapital = netWorth > 0.5;
  const targetAllocation = portfolio?.goal?.targetAllocation ?? {};
  const targetRows = Object.entries(targetAllocation)
    .filter(([, value]) => (value ?? 0) > 0)
    .sort((a, b) => (b[1] ?? 0) - (a[1] ?? 0));

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Portfolio telemetry
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <BarChart3 className="h-5 w-5 text-accent-agent" />
          Analytics
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          A compact read on value, wallet cash, targets, market context, and
          decision quality.
        </p>
      </div>

      <div className="grid gap-3 md:grid-cols-4">
        <MetricCard
          icon={CircleDollarSign}
          label="Net worth"
          value={formatCurrency(netWorth)}
          detail={`${formatCurrency(investedUsd)} invested · ${formatCurrency(idleCashUsd)} in wallet`}
          tone="pnl"
        />
        <MetricCard
          icon={PieChart}
          label="PnL"
          value={formatCurrency(portfolio?.totalPnlUsd ?? 0)}
          detail={formatPercent(portfolio?.totalPnlPct ?? 0)}
          tone={(portfolio?.totalPnlUsd ?? 0) >= 0 ? "pnl" : "risk"}
        />
        <MetricCard
          icon={ShieldCheck}
          label="Risk score"
          value={
            portfolio
              ? hasConfirmedCapital
                ? `${portfolio.riskScore}/100`
                : "--"
              : "--"
          }
          detail={
            portfolio
              ? hasConfirmedCapital
                ? (portfolio.goal?.riskTolerance ?? "Goal set")
                : "available after first approved move"
              : "No portfolio"
          }
          tone="agent"
        />
        <MetricCard
          icon={Brain}
          label={hasConfirmedCapital ? "Decision confidence" : "Current state"}
          value={
            hasConfirmedCapital && decisions.length
              ? `${Math.round(avgConfidence * 100)}%`
              : "--"
          }
          detail={
            hasConfirmedCapital
              ? `${decisions.length} ${decisions.length === 1 ? "decision" : "decisions"} loaded`
              : decisions.length
                ? `${decisions.length} previous ${decisions.length === 1 ? "review" : "reviews"} saved`
                : "No approved moves yet"
          }
          tone="agent"
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">
              Target allocation
            </span>
            <span className="text-[11px] font-mono text-text-lo">
              {portfolio?.goal ? "Goal weights" : "No goal"}
            </span>
          </BrutalCardHeader>
          <BrutalCardBody className="space-y-3">
            {targetRows.length ? (
              targetRows.map(([symbol, weight]) => (
                <div key={symbol}>
                  <div className="mb-1 flex items-center justify-between text-xs font-mono">
                    <span className="text-text-hi">{symbol}</span>
                    <span className="text-text-lo tabular-nums">{weight}%</span>
                  </div>
                  <div className="h-2 border border-border-default bg-bg">
                    <div
                      className="h-full bg-accent-pnl"
                      style={{
                        width: `${Math.min(100, Number(weight) || 0)}%`,
                      }}
                    />
                  </div>
                </div>
              ))
            ) : (
              <p className="text-xs font-mono text-text-lo">
                Create a portfolio goal to populate allocation analytics.
              </p>
            )}
          </BrutalCardBody>
        </BrutalCard>

        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">
              Market context
            </span>
            <BrutalPill tone="agent">
              {regime.current.replace("_", " ")}
            </BrutalPill>
          </BrutalCardHeader>
          <BrutalCardBody className="space-y-3 text-xs font-mono">
            <Row
              label="Fear & greed"
              value={snapshot ? `${snapshot.fearGreedIndex}/100` : "--"}
            />
            <Row
              label="BTC dominance"
              value={
                snapshot
                  ? hasBtcDominance
                    ? `${snapshot.btcDominance.toFixed(1)}%`
                    : "Unavailable"
                  : "--"
              }
              muted={snapshot ? !hasBtcDominance : true}
            />
            <Row
              label="Market cap"
              value={
                snapshot
                  ? hasMarketCap
                    ? formatCurrency(snapshot.totalMarketCapUsd, {
                        compact: true,
                      })
                    : "Unavailable"
                  : "--"
              }
              muted={snapshot ? !hasMarketCap : true}
            />
            <Row
              label="Snapshot"
              value={snapshot ? timeAgo(snapshot.capturedAt) : "Waiting"}
            />
            <p className="border-t border-border-default pt-3 text-[11px] leading-relaxed text-text-mut">
              Market data via CoinGecko
              {snapshot ? ` · updated ${timeAgo(snapshot.capturedAt)}` : ""}.
              Live prices refresh separately; some aggregate fields may show
              unavailable when the provider does not supply them.
            </p>
          </BrutalCardBody>
        </BrutalCard>
      </div>
    </div>
  );
}

function MetricCard({
  icon: Icon,
  label,
  value,
  detail,
  tone,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: string;
  detail: string;
  tone: "pnl" | "agent" | "risk";
}) {
  const color =
    tone === "pnl"
      ? "text-accent-pnl"
      : tone === "risk"
        ? "text-risk"
        : "text-accent-agent";
  return (
    <BrutalCard>
      <BrutalCardBody>
        <div className="flex items-center justify-between gap-3">
          <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
            {label}
          </p>
          <Icon className={`h-4 w-4 ${color}`} />
        </div>
        <p
          className={`mt-3 text-2xl font-mono font-semibold tabular-nums ${color}`}
        >
          {value}
        </p>
        <p className="mt-1 text-[11px] font-mono text-text-lo">{detail}</p>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function Row({
  label,
  value,
  muted = false,
}: {
  label: string;
  value: string;
  muted?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-text-lo">{label}</span>
      <span
        className={"tabular-nums " + (muted ? "text-text-mut" : "text-text-hi")}
      >
        {value}
      </span>
    </div>
  );
}
