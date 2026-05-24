import type { Metadata } from "next";
import { notFound } from "next/navigation";
import Link from "next/link";
import type { LucideIcon } from "lucide-react";
import {
  ArrowLeft,
  ArrowRight,
  Brain,
  ExternalLink,
  ShieldCheck,
  Target,
  WalletCards,
} from "lucide-react";
import { BrutalPill, ModelBadge, ProvenanceLine } from "@aegis/ui";
import { DEMO_BUNDLES, DEMO_SLUGS } from "@/lib/explore-data";
import { pageMetadata } from "@/lib/seo";
import { cn } from "@/lib/utils";
import type { AgentDecision } from "@/types";

interface PageProps {
  params: Promise<{ portfolioId: string }>;
}

export async function generateStaticParams() {
  return DEMO_SLUGS.map((portfolioId) => ({ portfolioId }));
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { portfolioId } = await params;
  const bundle = DEMO_BUNDLES[portfolioId];
  if (!bundle) return { title: "Explore — Aegis" };
  return pageMetadata({
    title: `${bundle.portfolio.name} (demo) — Aegis`,
    description: `Demo portfolio: ${bundle.portfolio.name}. See how the Aegis agent reasons about regime, allocation, and risk.`,
    path: `/explore/${portfolioId}`,
  });
}

export default async function ExploreDemoPage({ params }: PageProps) {
  const { portfolioId } = await params;
  const bundle = DEMO_BUNDLES[portfolioId];
  if (!bundle) notFound();

  const { portfolio, decisions, unsupportedSleeves = [] } = bundle;
  const goal = portfolio.goal;
  const decision = decisions[0];
  const readiness =
    unsupportedSleeves.length > 0 ? "simulation sleeves" : "live route ready";

  return (
    <div className="mx-auto w-full max-w-[1400px] space-y-5 md:space-y-6">
      <section className="border-brutal border-border-default bg-surface">
        <div className="grid gap-4 border-b border-border-default px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-start md:px-5">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <ShieldCheck className="h-4 w-4 text-accent-agent" />
              <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
                Demo portfolio
              </p>
              <BrutalPill tone="agent">READ ONLY</BrutalPill>
            </div>
            <h1 className="mt-2 font-mono text-2xl font-semibold text-text-hi md:text-3xl">
              {portfolio.name}
            </h1>
            <p className="mt-2 max-w-3xl text-sm leading-relaxed text-text-lo">
              {portfolioThesis(goal?.riskTolerance)} The page is inspect-only;
              real portfolios require wallet setup and explicit approval before
              money moves.
            </p>
            <div className="mt-2">
              <ProvenanceLine source="curated demo data" freshness="static" />
            </div>
          </div>
          <div className="flex flex-col gap-2 sm:flex-row md:flex-col lg:flex-row">
            <Link
              href="/explore"
              className="inline-flex min-h-10 items-center justify-center gap-2 border border-border-default bg-bg px-3 font-mono text-xs font-semibold text-text-lo hover:border-accent-agent hover:text-accent-agent"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              Back to demos
            </Link>
            <Link
              href="/login?next=%2Fdashboard"
              className="inline-flex min-h-10 items-center justify-center gap-2 border-brutal border-black bg-accent-pnl px-4 font-mono text-xs font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
            >
              Create real portfolio
              <ExternalLink className="h-3.5 w-3.5" />
            </Link>
          </div>
        </div>

        <div className="grid grid-cols-2 border-b border-border-default lg:grid-cols-4">
          <Metric
            icon={WalletCards}
            label="Portfolio value"
            value={`$${portfolio.totalValueUsd.toLocaleString()}`}
            detail="demo balance"
          />
          <Metric
            icon={WalletCards}
            label="All-time PnL"
            value={`${portfolio.totalPnlPct >= 0 ? "+" : ""}${portfolio.totalPnlPct.toFixed(1)}%`}
            detail={`$${portfolio.totalPnlUsd.toLocaleString()}`}
            tone={portfolio.totalPnlPct >= 0 ? "pnl" : "risk"}
          />
          <Metric
            icon={Target}
            label="Risk posture"
            value={goal?.riskTolerance ?? "demo"}
            detail={goal?.horizon ?? "n/a"}
          />
          <Metric
            icon={ShieldCheck}
            label="Readiness"
            value={readiness}
            detail="approval stays disabled"
            tone={unsupportedSleeves.length > 0 ? "warn" : "pnl"}
          />
        </div>
      </section>

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_380px]">
        <section className="border-brutal border-border-default bg-surface">
          <SectionHeader
            icon={Target}
            title="Target allocation"
            detail="demo target weights"
          />
          <div className="p-4 md:p-5">
            <AllocationBar targetAllocation={goal?.targetAllocation} />
            <div className="mt-5 grid gap-3 font-mono text-xs md:grid-cols-3">
              <Fact label="Horizon" value={goal?.horizon ?? "n/a"} />
              <Fact label="Objective" value={goal?.objective ?? "n/a"} />
              <Fact
                label="Risk tolerance"
                value={goal?.riskTolerance ?? "n/a"}
              />
            </div>
          </div>
        </section>

        <section className="border-brutal border-border-default bg-surface">
          <SectionHeader
            icon={ShieldCheck}
            title="Execution scope"
            detail="what this demo can show"
          />
          <div className="space-y-3 p-4 md:p-5">
            <Fact
              label="Mode"
              value="read-only"
              tone="agent"
              body="You can inspect reasoning and target mix without a session."
            />
            <Fact
              label="Approval"
              value="disabled"
              tone="warn"
              body="No demo click can submit a route or move funds."
            />
            <Fact
              label="Unsupported sleeves"
              value={
                unsupportedSleeves.length > 0
                  ? unsupportedSleeves.join(" / ")
                  : "none"
              }
              tone={unsupportedSleeves.length > 0 ? "warn" : "pnl"}
              body={
                unsupportedSleeves.length > 0
                  ? "Shown for product direction, not live execution."
                  : "Every target sleeve shown here is represented by the route registry."
              }
            />
          </div>
        </section>
      </div>

      <section className="border-brutal border-border-default bg-surface">
        <SectionHeader
          icon={Brain}
          title="Agent reasoning"
          detail="model, confidence, and critic"
        />
        <div className="divide-y divide-border-default">
          {decisions.map((entry, index) => (
            <DecisionCard key={entry.id} decision={entry} index={index + 1} />
          ))}
        </div>
      </section>

      <div className="flex flex-col gap-3 border border-accent-agent/30 bg-accent-agent/5 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="font-mono text-xs text-text-lo">
          {decision?.recommendation.summary ??
            "Open another demo to compare agent behavior."}
        </p>
        <Link
          href="/explore"
          className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 border border-accent-agent/40 bg-bg px-4 font-mono text-xs font-semibold text-accent-agent hover:border-accent-agent"
        >
          Compare demos
          <ArrowRight className="h-3.5 w-3.5" />
        </Link>
      </div>
    </div>
  );
}

function DecisionCard({
  decision,
  index,
}: {
  decision: AgentDecision;
  index: number;
}) {
  return (
    <article className="p-4 md:p-5">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-mono text-[11px] text-text-mut">
          #{index.toString().padStart(2, "0")}
        </span>
        <BrutalPill tone={regimeTone(decision.regime)}>
          {decision.regime?.replace("_", "-").toUpperCase() ?? "NEUTRAL"}
        </BrutalPill>
        {decision.modelSlug && <ModelBadge model={decision.modelSlug} />}
      </div>
      <p className="mt-3 text-sm font-semibold leading-relaxed text-text-hi">
        {decision.recommendation.summary}
      </p>
      <p className="mt-2 text-xs leading-relaxed text-text-lo">
        {decision.reasoning}
      </p>
      <ConfidenceBar confidence={decision.confidence ?? 0} />
      {decision.criticVerdict && (
        <div className="mt-4 border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 font-mono text-[11px] text-text-lo">
          <span className="text-accent-agent">Critic approved</span>{" "}
          <span className="text-text-mut">
            ({Math.round((decision.criticVerdict.confidence ?? 0) * 100)}%):
          </span>{" "}
          {decision.criticVerdict.notes}
        </div>
      )}
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

function Metric({
  detail,
  icon: Icon,
  label,
  tone = "default",
  value,
}: {
  detail: string;
  icon: LucideIcon;
  label: string;
  tone?: "default" | "pnl" | "agent" | "warn" | "risk";
  value: string;
}) {
  return (
    <div className="min-h-24 border-r border-border-default px-4 py-4 last:border-r-0 odd:border-b even:border-b md:px-5 lg:border-b-0">
      <div className="flex items-center gap-2">
        <Icon className={cn("h-4 w-4 shrink-0", toneClass(tone))} />
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p
        className={cn("mt-3 font-mono text-xl font-semibold", toneClass(tone))}
      >
        {value}
      </p>
      <p className="mt-1 font-mono text-[10px] text-text-mut">{detail}</p>
    </div>
  );
}

function Fact({
  body,
  label,
  tone = "default",
  value,
}: {
  body?: string;
  label: string;
  tone?: "default" | "pnl" | "agent" | "warn";
  value: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-3 py-2">
      <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className={cn("mt-1 font-mono font-semibold", toneClass(tone))}>
        {value}
      </p>
      {body && (
        <p className="mt-2 text-xs leading-relaxed text-text-lo">{body}</p>
      )}
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
      <div className="flex h-5 overflow-hidden border border-border-default bg-bg">
        {entries.map(([symbol, pct]) => (
          <div
            key={symbol}
            className={cn("h-full min-w-1", tokenColor(symbol))}
            style={{ flexBasis: `${pct}%` }}
            title={`${symbol} ${pct}%`}
          />
        ))}
      </div>
      <div className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {entries.map(([symbol, pct]) => (
          <div
            key={symbol}
            className="flex items-center justify-between gap-3 border border-border-default bg-bg px-3 py-2 font-mono text-xs"
          >
            <span className="inline-flex items-center gap-2 text-text-hi">
              <span className={cn("h-2.5 w-2.5", tokenColor(symbol))} />
              {symbol}
            </span>
            <span className="tabular-nums text-text-lo">{pct}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ConfidenceBar({ confidence }: { confidence: number }) {
  const pct = Math.max(0, Math.min(100, Math.round(confidence * 100)));
  return (
    <div className="mt-4">
      <div className="h-1.5 border border-border-default bg-bg">
        <div className="h-full bg-accent-agent" style={{ width: `${pct}%` }} />
      </div>
      <p className="mt-1 font-mono text-[10px] text-text-mut">
        confidence {pct}%
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

function tokenColor(symbol: string) {
  const normalized = symbol.toUpperCase();
  if (["USDC", "USYC"].includes(normalized)) return "bg-accent-pnl";
  if (["BTC", "CBBTC"].includes(normalized)) return "bg-text-hi";
  if (normalized === "ETH") return "bg-warn";
  if (["SOL", "LINK", "UNI"].includes(normalized)) return "bg-risk";
  if (normalized === "EURC") return "bg-text-mut";
  return "bg-raised";
}

function toneClass(tone: "default" | "pnl" | "agent" | "warn" | "risk") {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "warn") return "text-warn";
  if (tone === "risk") return "text-risk";
  return "text-text-hi";
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
