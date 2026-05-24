import type { Metadata } from "next";
import Link from "next/link";
import type { LucideIcon } from "lucide-react";
import {
  Activity,
  ArrowRight,
  BarChart3,
  Brain,
  CheckCircle2,
  Database,
  GitBranch,
  ShieldCheck,
  TimerReset,
} from "lucide-react";
import { LandingShell } from "@/components/layout/landing-shell";
import { BrutalPill, ModelBadge, ProvenanceLine } from "@aegis/ui";
import { pageMetadata } from "@/lib/seo";
import { cn } from "@/lib/utils";

export const metadata: Metadata = pageMetadata({
  title: "Regime Classifier Model Card — Aegis",
  description:
    "Backtest precision/recall numbers for the Aegis market-regime classifier. Trust signal, not marketing.",
  path: "/about/regime",
});

// Re-fetch on every request so the page reflects the latest persisted eval.
export const dynamic = "force-dynamic";
export const revalidate = 0;

type Regime = "risk_on" | "neutral" | "risk_off";

const REGIME_ORDER: Regime[] = ["risk_on", "neutral", "risk_off"];

const REGIME_LABEL: Record<Regime, string> = {
  risk_on: "RISK-ON",
  neutral: "NEUTRAL",
  risk_off: "RISK-OFF",
};

const REGIME_TONE: Record<Regime, "pnl" | "neutral" | "risk"> = {
  risk_on: "pnl",
  neutral: "neutral",
  risk_off: "risk",
};

const REGIME_COPY: Record<
  Regime,
  { title: string; decision: string; description: string }
> = {
  risk_on: {
    title: "Let growth sleeves breathe",
    decision: "Higher drift tolerance before trimming winners.",
    description:
      "The agent can tolerate more BTC/ETH/SOL exposure when momentum and drawdown features support a constructive market.",
  },
  neutral: {
    title: "Keep the target mix honest",
    decision: "Rebalance only when drift or idle cash justifies it.",
    description:
      "Neutral avoids over-trading. The strategist focuses on controlled moves, cash deployment, and portfolio concentration.",
  },
  risk_off: {
    title: "Defend drawdown first",
    decision: "Reduce volatile exposure and preserve cash optionality.",
    description:
      "Risk-off pushes the strategist toward stablecoin buffers, lower beta, and stronger approval language before movement.",
  },
};

const FEATURE_ROWS = [
  {
    label: "BTC 30d realized volatility",
    value: "stress proxy",
    source: "price_history",
  },
  {
    label: "90d cross-asset correlation",
    value: "crowding proxy",
    source: "price_history",
  },
  {
    label: "30d drawdown",
    value: "trend damage",
    source: "price_history",
  },
  {
    label: "Backward-looking window only",
    value: "no future leakage",
    source: "harness guardrail",
  },
];

const PIPELINE = [
  {
    icon: Database,
    label: "Price history",
    body: "BTC, ETH, stablecoin, and route-market features are read from persisted market data.",
  },
  {
    icon: Activity,
    label: "Regime label",
    body: "The classifier emits RISK-ON, NEUTRAL, or RISK-OFF with a confidence signal.",
  },
  {
    icon: Brain,
    label: "Strategist prompt",
    body: "The selected regime changes target drift, cash posture, and how aggressive a proposal can be.",
  },
  {
    icon: ShieldCheck,
    label: "Approval gate",
    body: "The critic and user approval still gate every executable movement.",
  },
];

interface PerRegime {
  precision: number;
  recall: number;
  f1: number;
  support: number;
}

interface Confusion {
  rows: number[][];
  labels: Regime[];
}

interface EvaluationRow {
  id: string;
  modelSlug: string;
  evalRunId: string;
  task: string;
  periodStart: string;
  periodEnd: string;
  samplesCount: number;
  accuracy: number | null;
  precisionMacro: number | null;
  recallMacro: number | null;
  f1Macro: number | null;
  brierScore: number | null;
  confusionJsonb: Confusion;
  perRegimeJsonb: Record<Regime, PerRegime>;
  createdAt: string;
}

interface LatestResponse {
  evaluation: EvaluationRow | null;
}

async function fetchLatest(): Promise<EvaluationRow | null> {
  const apiBase = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
  try {
    const res = await fetch(`${apiBase}/about/regime/latest`, {
      cache: "no-store",
    });
    if (!res.ok) return null;
    const body = (await res.json()) as LatestResponse;
    return body.evaluation;
  } catch {
    return null;
  }
}

export default async function RegimeModelCardPage() {
  const evaluation = await fetchLatest();
  const pageRenderedAt = new Date().toUTCString();

  return (
    <LandingShell width="wide">
      <div className="space-y-6">
        <section className="border-brutal border-border-default bg-surface">
          <div className="grid gap-5 border-b border-border-default px-4 py-5 md:grid-cols-[minmax(0,1fr)_auto] md:px-5">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <Activity className="h-4 w-4 text-accent-agent" />
                <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
                  Model card
                </p>
                <BrutalPill tone={evaluation ? "pnl" : "warn"}>
                  {evaluation ? "EVIDENCE LIVE" : "EVIDENCE PENDING"}
                </BrutalPill>
                {evaluation?.modelSlug && (
                  <ModelBadge model={evaluation.modelSlug} />
                )}
              </div>
              <h1 className="mt-3 font-mono text-3xl font-semibold text-text-hi md:text-4xl">
                Regime classifier
              </h1>
              <p className="mt-3 max-w-3xl text-sm leading-relaxed text-text-lo">
                Aegis uses a market-regime classifier to decide whether the
                strategist should lean into growth, stay close to target, or
                defend capital. This page describes the contract and publishes
                evaluation evidence when the harness has enough samples.
              </p>
              <div className="mt-3">
                <ProvenanceLine
                  source={
                    evaluation
                      ? "Aegis backtest harness"
                      : "Aegis model-card contract"
                  }
                  freshness={
                    evaluation
                      ? `${evaluation.samplesCount} samples`
                      : "awaiting first eval"
                  }
                />
              </div>
            </div>
            <div className="grid min-w-[220px] gap-2 font-mono text-xs">
              <StatusFact
                label="Model"
                value={evaluation?.modelSlug ?? "classifier configured"}
                tone="agent"
              />
              <StatusFact
                label="Evidence"
                value={evaluation ? "published" : "pending sample floor"}
                tone={evaluation ? "pnl" : "warn"}
              />
              <StatusFact label="Outputs" value="3 regimes" />
            </div>
          </div>

          <div className="grid grid-cols-2 border-b border-border-default lg:grid-cols-4">
            <Metric
              icon={CheckCircle2}
              label="Accuracy"
              value={formatPct(evaluation?.accuracy)}
              detail={evaluation ? "latest run" : "awaiting eval"}
              tone={evaluation ? "pnl" : "warn"}
            />
            <Metric
              icon={BarChart3}
              label="Macro F1"
              value={formatPct(evaluation?.f1Macro)}
              detail={evaluation ? "balanced score" : "sample floor"}
              tone={evaluation ? "agent" : "warn"}
            />
            <Metric
              icon={TimerReset}
              label="Eval period"
              value={
                evaluation
                  ? `${shortDate(evaluation.periodStart)} → ${shortDate(evaluation.periodEnd)}`
                  : "not published"
              }
              detail={evaluation ? "historical replay" : "first cycle pending"}
            />
            <Metric
              icon={GitBranch}
              label="Run id"
              value={evaluation ? evaluation.evalRunId.slice(0, 8) : "queued"}
              detail={`rendered ${pageRenderedAt}`}
              tone="agent"
            />
          </div>
        </section>

        <section className="grid gap-4 lg:grid-cols-3">
          {REGIME_ORDER.map((regime) => (
            <RegimeCard key={regime} regime={regime} />
          ))}
        </section>

        <section className="grid gap-5 lg:grid-cols-[minmax(0,1.2fr)_minmax(360px,0.8fr)]">
          <Panel
            icon={Database}
            title="Classifier inputs"
            detail="backward-looking features only"
          >
            <div className="grid gap-2">
              {FEATURE_ROWS.map((row) => (
                <div
                  key={row.label}
                  className="grid gap-2 border border-border-default bg-bg px-3 py-2 font-mono text-xs sm:grid-cols-[minmax(0,1fr)_130px_130px]"
                >
                  <span className="text-text-hi">{row.label}</span>
                  <span className="text-text-lo">{row.value}</span>
                  <span className="text-text-mut">via {row.source}</span>
                </div>
              ))}
            </div>
          </Panel>

          <Panel
            icon={ShieldCheck}
            title="Operating boundaries"
            detail="what the regime cannot do"
          >
            <ul className="space-y-3 text-sm leading-relaxed text-text-lo">
              <li>
                <span className="font-mono text-accent-agent">
                  Signal, not execution.
                </span>{" "}
                The classifier changes the strategist posture; it never moves
                funds by itself.
              </li>
              <li>
                <span className="font-mono text-accent-agent">
                  No hidden auto-trading.
                </span>{" "}
                The critic and user approval remain the final gates.
              </li>
              <li>
                <span className="font-mono text-accent-agent">
                  Evidence updates live.
                </span>{" "}
                Metrics switch from pending to published once the backend has a
                completed evaluation row.
              </li>
            </ul>
          </Panel>
        </section>

        <Panel
          icon={Brain}
          title="Decision pipeline"
          detail="where the model changes behavior"
        >
          <div className="grid gap-3 lg:grid-cols-4">
            {PIPELINE.map((step, index) => (
              <PipelineStep key={step.label} step={step} index={index + 1} />
            ))}
          </div>
        </Panel>

        {evaluation ? (
          <div className="grid gap-5 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
            <PerRegimeTable perRegime={evaluation.perRegimeJsonb} />
            <ConfusionMatrix confusion={evaluation.confusionJsonb} />
          </div>
        ) : (
          <PendingEvidence />
        )}

        <div className="flex flex-col gap-3 border border-accent-agent/30 bg-accent-agent/5 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
          <p className="font-mono text-xs text-text-lo">
            The demo portfolios show how this signal changes recommendation
            posture before any approval modal appears.
          </p>
          <div className="flex flex-col gap-2 sm:flex-row">
            <LinkButton href="/about/regime/backtest" tone="agent">
              Open backtest evidence
            </LinkButton>
            <LinkButton href="/explore" tone="pnl">
              Explore demo portfolios
            </LinkButton>
          </div>
        </div>
      </div>
    </LandingShell>
  );
}

function RegimeCard({ regime }: { regime: Regime }) {
  const copy = REGIME_COPY[regime];
  return (
    <article className="border-brutal border-border-default bg-surface p-4">
      <div className="flex items-center justify-between gap-3">
        <BrutalPill tone={REGIME_TONE[regime]}>
          {REGIME_LABEL[regime]}
        </BrutalPill>
        <span className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          output
        </span>
      </div>
      <h2 className="mt-4 font-mono text-lg font-semibold text-text-hi">
        {copy.title}
      </h2>
      <p className="mt-2 font-mono text-xs text-accent-agent">
        {copy.decision}
      </p>
      <p className="mt-3 text-sm leading-relaxed text-text-lo">
        {copy.description}
      </p>
    </article>
  );
}

function PendingEvidence() {
  return (
    <section className="border-brutal border-warn/45 bg-warn/5">
      <div className="grid gap-4 border-b border-warn/30 px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:px-5">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-warn">
            Evaluation pending
          </p>
          <h2 className="mt-2 font-mono text-xl font-semibold text-text-hi">
            The model card is ready; live metrics are waiting for the first
            evaluation row.
          </h2>
          <p className="mt-2 max-w-3xl text-sm leading-relaxed text-text-lo">
            Precision, recall, Brier score, and confusion-matrix cells will
            populate here after the evaluation cycle records enough labeled
            samples. The placeholder below shows exactly what will appear.
          </p>
        </div>
        <LinkButton href="/explore" tone="agent">
          See regime in demos
        </LinkButton>
      </div>
      <div className="grid gap-0 md:grid-cols-4">
        {["Accuracy", "Precision", "Recall", "Macro F1"].map((label) => (
          <div
            key={label}
            className="border-b border-r border-warn/20 px-4 py-4 last:border-r-0 md:border-b-0"
          >
            <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
              {label}
            </p>
            <p className="mt-2 font-mono text-2xl font-semibold text-warn">
              pending
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

function Panel({
  children,
  detail,
  icon: Icon,
  title,
}: {
  children: React.ReactNode;
  detail: string;
  icon: LucideIcon;
  title: string;
}) {
  return (
    <section className="border-brutal border-border-default bg-surface">
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
      <div className="p-4 md:p-5">{children}</div>
    </section>
  );
}

function PipelineStep({
  index,
  step,
}: {
  index: number;
  step: { icon: LucideIcon; label: string; body: string };
}) {
  const Icon = step.icon;
  return (
    <div className="border border-border-default bg-bg px-3 py-3">
      <div className="flex items-center justify-between gap-3">
        <Icon className="h-4 w-4 text-accent-agent" />
        <span className="font-mono text-[10px] text-text-mut">
          {index.toString().padStart(2, "0")}
        </span>
      </div>
      <p className="mt-3 font-mono text-sm font-semibold text-text-hi">
        {step.label}
      </p>
      <p className="mt-2 text-xs leading-relaxed text-text-lo">{step.body}</p>
    </div>
  );
}

function PerRegimeTable({
  perRegime,
}: {
  perRegime: Record<Regime, PerRegime>;
}) {
  return (
    <Panel
      icon={BarChart3}
      title="Per-regime breakdown"
      detail="precision, recall, support"
    >
      <div className="overflow-x-auto">
        <table className="w-full font-mono text-xs">
          <thead>
            <tr className="border-b border-border-default text-[10px] uppercase tracking-widest text-text-mut">
              <th className="px-3 py-2 text-left">Regime</th>
              <th className="px-3 py-2 text-right">Precision</th>
              <th className="px-3 py-2 text-right">Recall</th>
              <th className="px-3 py-2 text-right">F1</th>
              <th className="px-3 py-2 text-right">Support</th>
            </tr>
          </thead>
          <tbody>
            {REGIME_ORDER.map((regime) => {
              const metric = perRegime?.[regime];
              if (!metric) return null;
              return (
                <tr
                  key={regime}
                  className="border-b border-border-default/50 last:border-0"
                >
                  <td className="px-3 py-2">
                    <BrutalPill tone={REGIME_TONE[regime]}>
                      {REGIME_LABEL[regime]}
                    </BrutalPill>
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {formatPct(metric.precision)}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {formatPct(metric.recall)}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {formatPct(metric.f1)}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums text-text-lo">
                    {metric.support}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}

function ConfusionMatrix({ confusion }: { confusion: Confusion }) {
  const labels = confusion.labels ?? REGIME_ORDER;
  const rows = confusion.rows ?? [];

  return (
    <Panel
      icon={GitBranch}
      title="Confusion matrix"
      detail="actual rows, predicted columns"
    >
      <div className="overflow-x-auto">
        <table className="w-full font-mono text-xs">
          <thead>
            <tr className="border-b border-border-default text-[10px] uppercase tracking-widest text-text-mut">
              <th className="px-3 py-2 text-left">Actual</th>
              {labels.map((label) => (
                <th key={label} className="px-3 py-2 text-right">
                  {REGIME_LABEL[label]}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, rowIndex) => {
              const label = labels[rowIndex] ?? REGIME_ORDER[rowIndex]!;
              return (
                <tr
                  key={label}
                  className="border-b border-border-default/50 last:border-0"
                >
                  <td className="px-3 py-2 text-text-hi">
                    {REGIME_LABEL[label]}
                  </td>
                  {row.map((cell, columnIndex) => (
                    <td
                      key={`${label}-${columnIndex}`}
                      className={cn(
                        "px-3 py-2 text-right tabular-nums",
                        rowIndex === columnIndex
                          ? "font-semibold text-accent-pnl"
                          : "text-text-lo",
                      )}
                    >
                      {cell}
                    </td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Panel>
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
  tone?: "default" | "pnl" | "agent" | "warn";
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
        className={cn(
          "mt-3 break-words font-mono text-xl font-semibold tabular-nums",
          toneClass(tone),
        )}
      >
        {value}
      </p>
      <p className="mt-1 line-clamp-2 font-mono text-[10px] text-text-mut">
        {detail}
      </p>
    </div>
  );
}

function StatusFact({
  label,
  tone = "default",
  value,
}: {
  label: string;
  tone?: "default" | "pnl" | "agent" | "warn";
  value: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className={cn("mt-1 break-words font-semibold", toneClass(tone))}>
        {value}
      </p>
    </div>
  );
}

function LinkButton({
  children,
  href,
  tone,
}: {
  children: React.ReactNode;
  href: string;
  tone: "agent" | "pnl";
}) {
  return (
    <Link
      href={href}
      className={cn(
        "inline-flex min-h-10 items-center justify-center gap-2 border px-4 font-mono text-xs font-semibold",
        tone === "pnl"
          ? "border-black bg-accent-pnl text-black shadow-brutal-sm hover:shadow-brutal"
          : "border-accent-agent/40 bg-bg text-accent-agent hover:border-accent-agent",
      )}
    >
      {children}
      <ArrowRight className="h-3.5 w-3.5" />
    </Link>
  );
}

function toneClass(tone: "default" | "pnl" | "agent" | "warn") {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "warn") return "text-warn";
  return "text-text-hi";
}

function formatPct(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return "pending";
  }
  return `${(value * 100).toFixed(1)}%`;
}

function shortDate(value: string) {
  return new Date(value).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "2-digit",
  });
}
