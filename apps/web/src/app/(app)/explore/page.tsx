import type { Metadata } from "next";
import Link from "next/link";
import type { LucideIcon } from "lucide-react";
import {
  Activity,
  ArrowRight,
  BarChart3,
  BookOpen,
  Brain,
  Clock3,
  ShieldCheck,
  WalletCards,
} from "lucide-react";
import { BrutalPill, ModelBadge, ProvenanceLine } from "@aegis/ui";
import { DEMO_BUNDLES, type DemoBundle } from "@/lib/explore-data";
import { pageMetadata } from "@/lib/seo";
import { cn } from "@/lib/utils";
import type { AgentDecision } from "@/types";

export const metadata: Metadata = pageMetadata({
  title: "Explore Demo Portfolios — Aegis",
  description:
    "Three curated Aegis portfolios across different risk profiles and regimes — see how the agent reasons before signing up.",
  path: "/explore",
});

const GRID_COLS =
  "lg:grid-cols-[minmax(210px,1.15fr)_minmax(190px,1fr)_minmax(240px,1.2fr)_minmax(220px,1.1fr)_150px]";

export default function ExploreIndex() {
  const bundles = Object.values(DEMO_BUNDLES);
  const totalDemoValue = bundles.reduce(
    (sum, { portfolio }) => sum + portfolio.totalValueUsd,
    0,
  );
  const decisionCount = bundles.reduce(
    (sum, { decisions }) => sum + decisions.length,
    0,
  );
  const supportedCount = bundles.filter(
    ({ unsupportedSleeves = [] }) => unsupportedSleeves.length === 0,
  ).length;

  return (
    <div className="mx-auto w-full max-w-[1400px] space-y-5 md:space-y-6">
      <section className="border-brutal border-border-default bg-surface">
        <div className="grid gap-4 border-b border-border-default px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-start md:px-5">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <BookOpen className="h-4 w-4 text-accent-agent" />
              <h1 className="font-mono text-2xl font-semibold text-text-hi md:text-3xl">
                Explore demo portfolios
              </h1>
              <BrutalPill tone="agent">READ ONLY</BrutalPill>
            </div>
            <p className="mt-2 max-w-3xl text-sm leading-relaxed text-text-lo">
              Compare curated portfolios by risk posture, target mix, agent
              signal, and execution readiness. Every row opens a reasoning trace
              without requiring wallet setup.
            </p>
            <div className="mt-2">
              <ProvenanceLine source="curated demo data" freshness="static" />
            </div>
          </div>
          <Link
            href="/login?next=%2Fdashboard"
            className="inline-flex min-h-10 items-center justify-center gap-2 border-brutal border-black bg-accent-pnl px-4 py-2 font-mono text-xs font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
          >
            Create real portfolio
            <ArrowRight className="h-3.5 w-3.5" />
          </Link>
        </div>

        <div className="grid grid-cols-2 border-b border-border-default lg:grid-cols-4">
          <SummaryCell
            icon={WalletCards}
            label="Demo AUM"
            value={formatCompactUsd(totalDemoValue)}
            detail={`${bundles.length} profiles`}
            tone="pnl"
          />
          <SummaryCell
            icon={Brain}
            label="Agent decisions"
            value={String(decisionCount)}
            detail="model + critic"
            tone="agent"
          />
          <SummaryCell
            icon={ShieldCheck}
            label="Live-route demos"
            value={`${supportedCount}/${bundles.length}`}
            detail="no coming-soon sleeves"
            tone={supportedCount === bundles.length ? "pnl" : "warn"}
          />
          <SummaryCell
            icon={Clock3}
            label="Mode"
            value="read-only"
            detail="safe to inspect"
          />
        </div>
      </section>

      <section className="border-brutal border-border-default bg-surface">
        <SectionHeader
          icon={BarChart3}
          title="Portfolio comparison"
          detail="risk, allocation, signal, and readiness"
        />
        <div
          className={cn(
            "hidden border-b border-border-default bg-bg/60 px-4 py-3 font-mono text-[10px] uppercase tracking-widest text-text-mut md:px-5 lg:grid lg:gap-4",
            GRID_COLS,
          )}
        >
          <span>Profile</span>
          <span>Portfolio value</span>
          <span>Target mix</span>
          <span>Agent signal</span>
          <span className="text-right">Open</span>
        </div>
        <div>
          {bundles.map((bundle) => (
            <DemoRow key={bundle.portfolio.id} bundle={bundle} />
          ))}
        </div>
      </section>

      <section className="border-brutal border-border-default bg-surface">
        <SectionHeader
          icon={Activity}
          title="Latest reasoning"
          detail="one current decision from each demo"
        />
        <div className="grid divide-y divide-border-default lg:grid-cols-3 lg:divide-x lg:divide-y-0">
          {bundles.map((bundle) => (
            <DecisionPreview key={bundle.portfolio.id} bundle={bundle} />
          ))}
        </div>
      </section>
    </div>
  );
}

function DemoRow({ bundle }: { bundle: DemoBundle }) {
  const { portfolio, decisions, unsupportedSleeves = [] } = bundle;
  const decision = decisions[0];
  const goal = portfolio.goal;
  const readiness =
    unsupportedSleeves.length > 0 ? "simulation sleeves" : "live route ready";
  const readinessTone = unsupportedSleeves.length > 0 ? "warn" : "pnl";

  return (
    <Link
      href={`/explore/${portfolio.id}`}
      className={cn(
        "block border-b border-border-default px-4 py-4 last:border-b-0 hover:bg-raised md:px-5 lg:grid lg:items-center lg:gap-4",
        GRID_COLS,
      )}
    >
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <h2 className="font-mono text-base font-semibold text-text-hi">
            {portfolio.name}
          </h2>
          <BrutalPill tone={riskTone(goal?.riskTolerance)}>
            {goal?.riskTolerance ?? "demo"}
          </BrutalPill>
        </div>
        <p className="mt-2 text-xs leading-relaxed text-text-lo">
          {portfolioThesis(goal?.riskTolerance)}
        </p>
        <div className="mt-3 grid grid-cols-3 gap-2 font-mono text-[10px] lg:hidden">
          <MiniFact label="Horizon" value={goal?.horizon ?? "n/a"} />
          <MiniFact label="Objective" value={goal?.objective ?? "n/a"} />
          <MiniFact label="Readiness" value={readiness} tone={readinessTone} />
        </div>
      </div>

      <div className="mt-4 min-w-0 font-mono lg:mt-0">
        <p className="text-xl font-semibold tabular-nums text-text-hi">
          ${portfolio.totalValueUsd.toLocaleString()}
        </p>
        <p
          className={cn(
            "mt-1 text-xs tabular-nums",
            portfolio.totalPnlPct >= 0 ? "text-accent-pnl" : "text-risk",
          )}
        >
          {portfolio.totalPnlPct >= 0 ? "+" : ""}
          {portfolio.totalPnlPct.toFixed(1)}% all-time ·{" "}
          {goal?.horizon ?? "n/a"} horizon
        </p>
        <p className="mt-2 hidden text-[10px] uppercase tracking-widest text-text-mut lg:block">
          {goal?.objective ?? "demo"} objective
        </p>
      </div>

      <div className="mt-4 min-w-0 lg:mt-0">
        <AllocationBar targetAllocation={goal?.targetAllocation} />
        {unsupportedSleeves.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1">
            {unsupportedSleeves.map((sleeve) => (
              <span
                key={sleeve}
                className="border border-warn/40 bg-warn/5 px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wider text-warn"
              >
                {sleeve} coming soon
              </span>
            ))}
          </div>
        )}
      </div>

      <div className="mt-4 min-w-0 lg:mt-0">
        <div className="flex flex-wrap items-center gap-2">
          {decision?.regime && (
            <BrutalPill tone={regimeTone(decision.regime)}>
              {decision.regime.replace("_", "-").toUpperCase()}
            </BrutalPill>
          )}
          {decision?.modelSlug && <ModelBadge model={decision.modelSlug} />}
        </div>
        <ConfidenceBar confidence={decision?.confidence ?? 0} />
        <p className="mt-2 font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {readiness}
        </p>
      </div>

      <div className="mt-4 flex items-center justify-between gap-3 font-mono text-xs lg:mt-0 lg:justify-end">
        <span
          className={cn(
            "lg:hidden",
            readinessTone === "pnl" ? "text-accent-pnl" : "text-warn",
          )}
        >
          {readiness}
        </span>
        <span className="inline-flex items-center gap-1 text-accent-agent">
          Open trace
          <ArrowRight className="h-3.5 w-3.5" />
        </span>
      </div>
    </Link>
  );
}

function DecisionPreview({ bundle }: { bundle: DemoBundle }) {
  const { portfolio, decisions } = bundle;
  const decision = decisions[0];
  if (!decision) return null;

  return (
    <article className="min-w-0 p-4 md:p-5">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {portfolio.name}
        </span>
        <BrutalPill tone={regimeTone(decision.regime)}>
          {decision.regime?.replace("_", "-").toUpperCase()}
        </BrutalPill>
      </div>
      <p className="mt-3 text-sm font-semibold leading-relaxed text-text-hi">
        {decision.recommendation.summary}
      </p>
      <p className="mt-2 line-clamp-4 text-xs leading-relaxed text-text-lo">
        {decision.reasoning}
      </p>
      <div className="mt-4 flex flex-wrap items-center gap-2">
        {decision.modelSlug && <ModelBadge model={decision.modelSlug} />}
        <span className="font-mono text-[10px] text-text-mut">
          confidence {Math.round((decision.confidence ?? 0) * 100)}%
        </span>
      </div>
      <Link
        href={`/explore/${portfolio.id}`}
        className="mt-4 inline-flex items-center gap-1 font-mono text-xs text-accent-agent hover:underline"
      >
        Inspect reasoning
        <ArrowRight className="h-3.5 w-3.5" />
      </Link>
    </article>
  );
}

function SectionHeader({
  detail,
  icon: Icon,
  title,
}: {
  detail: string;
  icon: LucideIcon;
  title: string;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border-default px-4 py-3 md:px-5">
      <div className="flex items-center gap-2">
        <Icon className="h-4 w-4 text-accent-agent" />
        <h2 className="font-mono text-lg font-semibold text-text-hi">
          {title}
        </h2>
      </div>
      <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
        {detail}
      </p>
    </div>
  );
}

function SummaryCell({
  detail,
  icon: Icon,
  label,
  tone = "default",
  value,
}: {
  detail: string;
  icon: LucideIcon;
  label: string;
  tone?: "default" | "pnl" | "agent" | "warn";
  value: string;
}) {
  const toneClass =
    tone === "pnl"
      ? "text-accent-pnl"
      : tone === "agent"
        ? "text-accent-agent"
        : tone === "warn"
          ? "text-warn"
          : "text-text-hi";
  return (
    <div className="min-h-24 border-r border-border-default px-4 py-4 last:border-r-0 odd:border-b even:border-b md:px-5 lg:border-b-0">
      <div className={cn("flex items-center gap-2", toneClass)}>
        <Icon className="h-4 w-4 shrink-0" />
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p className={cn("mt-3 font-mono text-2xl font-semibold", toneClass)}>
        {value}
      </p>
      <p className="mt-1 font-mono text-[10px] text-text-mut">{detail}</p>
    </div>
  );
}

function AllocationBar({
  targetAllocation,
}: {
  targetAllocation: Record<string, number | undefined> | undefined;
}) {
  const entries = allocationEntries(targetAllocation);
  if (entries.length === 0) {
    return <p className="font-mono text-xs text-text-mut">No target set</p>;
  }

  return (
    <div>
      <div className="flex h-4 overflow-hidden border border-border-default bg-bg">
        {entries.map(([symbol, pct]) => (
          <div
            key={symbol}
            className={cn("h-full min-w-1", tokenColor(symbol))}
            style={{ flexBasis: `${pct}%` }}
            title={`${symbol} ${pct}%`}
          />
        ))}
      </div>
      <div className="mt-2 flex flex-wrap gap-1">
        {entries.map(([symbol, pct]) => (
          <span
            key={symbol}
            className="border border-border-default bg-bg px-1.5 py-0.5 font-mono text-[10px] text-text-lo"
          >
            <span
              className={cn("mr-1 inline-block h-2 w-2", tokenColor(symbol))}
            />
            {symbol} {pct}%
          </span>
        ))}
      </div>
    </div>
  );
}

function ConfidenceBar({ confidence }: { confidence: number }) {
  const pct = Math.max(0, Math.min(100, Math.round(confidence * 100)));
  return (
    <div className="mt-3">
      <div className="h-1.5 border border-border-default bg-bg">
        <div className="h-full bg-accent-agent" style={{ width: `${pct}%` }} />
      </div>
      <p className="mt-1 font-mono text-[10px] text-text-mut">
        confidence {pct}%
      </p>
    </div>
  );
}

function MiniFact({
  label,
  tone = "default",
  value,
}: {
  label: string;
  tone?: "default" | "pnl" | "warn";
  value: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-2 py-1.5">
      <p className="uppercase tracking-widest text-text-mut">{label}</p>
      <p
        className={cn(
          "mt-1 truncate font-semibold",
          tone === "pnl"
            ? "text-accent-pnl"
            : tone === "warn"
              ? "text-warn"
              : "text-text-hi",
        )}
      >
        {value}
      </p>
    </div>
  );
}

function allocationEntries(
  targetAllocation: Record<string, number | undefined> | undefined,
) {
  if (!targetAllocation) return [];
  return Object.entries(targetAllocation)
    .filter((entry): entry is [string, number] => (entry[1] ?? 0) > 0)
    .sort((a, b) => b[1] - a[1]);
}

function regimeTone(regime: AgentDecision["regime"]) {
  if (regime === "risk_on") return "pnl";
  if (regime === "risk_off") return "risk";
  return "neutral";
}

function riskTone(risk: string | undefined) {
  if (risk === "aggressive") return "risk";
  if (risk === "conservative") return "pnl";
  return "neutral";
}

function tokenColor(symbol: string) {
  const normalized = symbol.toUpperCase();
  if (["USDC", "USYC"].includes(normalized)) return "bg-accent-pnl";
  if (["BTC", "CBBTC"].includes(normalized)) return "bg-text-hi";
  if (normalized === "ETH") return "bg-warn";
  if (["SOL", "LINK", "UNI"].includes(normalized)) return "bg-risk";
  if (normalized === "EURC") return "bg-text-mut";
  return "bg-raised";
}

function portfolioThesis(risk: string | undefined) {
  if (risk === "conservative") {
    return "Yield and drawdown defense first; crypto beta is deliberately capped.";
  }
  if (risk === "aggressive") {
    return "Growth sleeve with higher drift tolerance before the agent trims risk.";
  }
  return "Treasury-style balance: stable yield, EUR exposure, and controlled BTC/ETH.";
}

function formatCompactUsd(value: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}
