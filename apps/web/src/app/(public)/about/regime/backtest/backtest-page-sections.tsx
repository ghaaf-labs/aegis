import {
  BarChart3,
  CalendarRange,
  CheckCircle2,
  GitCompareArrows,
  LineChart,
  ShieldAlert,
  Sparkles,
  type LucideIcon,
} from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
  ModelBadge,
  ProvenanceLine,
  type PillTone,
} from "@aegis/ui";
import { BacktestChart, type Sample } from "@/components/regime/backtest-chart";
import {
  BACKTEST_EVENTS,
  REGIME_LABEL,
  type Regime,
} from "./backtest-reference";

export interface FetchState {
  evalRunId: string | null;
  modelSlug: string | null;
  samples: Sample[];
  status: "live" | "empty" | "disabled" | "unreachable";
}

export interface Summary {
  agreement: number;
  end: string;
  perRegime: RegimeSummary[];
  samplesCount: number;
  start: string;
  transitions: number;
}

export interface RegimeSummary {
  actual: number;
  f1: number;
  label: Regime;
  precision: number;
  predicted: number;
  recall: number;
}

export function MetricStrip({
  live,
  summary,
}: {
  live: boolean;
  summary: Summary;
}) {
  return (
    <section className="grid border-[2px] border-border-default bg-surface md:grid-cols-4">
      <Metric
        icon={BarChart3}
        label="Samples"
        value={String(summary.samplesCount)}
      />
      <Metric
        icon={CheckCircle2}
        label="Agreement"
        value={formatPct(summary.agreement)}
        tone="pnl"
      />
      <Metric
        icon={GitCompareArrows}
        label="Regime shifts"
        value={String(summary.transitions)}
      />
      <Metric
        icon={CalendarRange}
        label={live ? "Published run" : "Preview window"}
        value={`${summary.start} -> ${summary.end}`}
      />
    </section>
  );
}

export function EvidenceGrid({
  fetched,
  live,
  modelSlug,
  runId,
  samples,
}: {
  fetched: FetchState;
  live: boolean;
  modelSlug: string;
  runId: string;
  samples: Sample[];
}) {
  return (
    <section className="mt-5 grid gap-5 lg:grid-cols-[minmax(0,1fr)_320px]">
      <BrutalCard variant="raised" className="min-w-0">
        <BrutalCardHeader className="flex-wrap gap-3">
          <div className="flex items-center gap-3">
            <LineChart className="h-4 w-4 text-accent-agent" />
            <div>
              <h2 className="font-mono text-sm font-semibold text-text-hi">
                Prediction replay
              </h2>
              <p className="mt-0.5 font-mono text-[10px] uppercase tracking-widest text-text-mut">
                predicted area · realized dotted line
              </p>
            </div>
          </div>
          <ProvenanceLine
            source={live ? "Aegis backtest harness" : "Aegis reference replay"}
            freshness={`run ${runId.slice(0, 18)}`}
          />
        </BrutalCardHeader>
        <BrutalCardBody>
          <BacktestChart samples={samples} />
        </BrutalCardBody>
      </BrutalCard>

      <PublicationState fetched={fetched} modelSlug={modelSlug} live={live} />
    </section>
  );
}

export function PerRegimeTable({ rows }: { rows: RegimeSummary[] }) {
  return (
    <BrutalCard className="min-w-0">
      <BrutalCardHeader>
        <span className="font-mono text-xs uppercase tracking-widest text-text-lo">
          Per-regime quality
        </span>
        <span className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          actual labels by 30d forward return
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="min-w-0 overflow-x-auto p-0">
        <table className="w-full min-w-[560px] font-mono text-xs">
          <thead>
            <tr className="border-b border-border-default text-[10px] uppercase tracking-widest text-text-mut">
              <th className="px-4 py-3 text-left">Regime</th>
              <th className="px-4 py-3 text-right">Actual</th>
              <th className="px-4 py-3 text-right">Predicted</th>
              <th className="px-4 py-3 text-right">Precision</th>
              <th className="px-4 py-3 text-right">Recall</th>
              <th className="px-4 py-3 text-right">F1</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.label}
                className="border-b border-border-default/50 last:border-0"
              >
                <td className="px-4 py-3">
                  <BrutalPill tone={regimeTone(row.label)}>
                    {REGIME_LABEL[row.label]}
                  </BrutalPill>
                </td>
                <td className="px-4 py-3 text-right tabular-nums">
                  {row.actual}
                </td>
                <td className="px-4 py-3 text-right tabular-nums">
                  {row.predicted}
                </td>
                <td className="px-4 py-3 text-right tabular-nums">
                  {formatPct(row.precision)}
                </td>
                <td className="px-4 py-3 text-right tabular-nums">
                  {formatPct(row.recall)}
                </td>
                <td className="px-4 py-3 text-right tabular-nums text-text-hi">
                  {formatPct(row.f1)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </BrutalCardBody>
    </BrutalCard>
  );
}

export function EventList() {
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="font-mono text-xs uppercase tracking-widest text-text-lo">
          Windows to inspect
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="grid gap-3">
        {BACKTEST_EVENTS.map((event) => (
          <div
            key={event.date}
            className="grid gap-2 border border-border-default bg-bg px-3 py-3 sm:grid-cols-[88px_minmax(0,1fr)]"
          >
            <p className="font-mono text-xs text-accent-agent">{event.date}</p>
            <div>
              <p className="font-mono text-sm font-semibold text-text-hi">
                {event.title}
              </p>
              <p className="mt-1 text-sm leading-relaxed text-text-lo">
                {event.body}
              </p>
            </div>
          </div>
        ))}
      </BrutalCardBody>
    </BrutalCard>
  );
}

export function MethodologyPanel() {
  return (
    <BrutalCard className="mt-5">
      <BrutalCardHeader>
        <span className="font-mono text-xs uppercase tracking-widest text-text-lo">
          Replay method
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="grid gap-3 md:grid-cols-4">
        <MethodStep
          step="1"
          title="Build features"
          body="Use only backward-looking price history: 30d vol, 90d correlation, and max drawdown."
        />
        <MethodStep
          step="2"
          title="Classify"
          body="Ask the regime model for one of RISK-ON, NEUTRAL, or RISK-OFF at each weekly window."
        />
        <MethodStep
          step="3"
          title="Realize"
          body="Bucket the next 30d BTC return: below -10%, inside the band, or above +10%."
        />
        <MethodStep
          step="4"
          title="Score"
          body="Compare predicted vs realized labels and publish sample-level evidence plus aggregate metrics."
        />
      </BrutalCardBody>
    </BrutalCard>
  );
}

function PublicationState({
  fetched,
  live,
  modelSlug,
}: {
  fetched: FetchState;
  live: boolean;
  modelSlug: string;
}) {
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="font-mono text-xs uppercase tracking-widest text-text-lo">
          Publication state
        </span>
        <ModelBadge model={modelSlug} />
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-4">
        <StatusLine
          icon={live ? CheckCircle2 : ShieldAlert}
          label="Sample source"
          tone={live ? "pnl" : "warn"}
          value={sourceLabel(fetched.status)}
        />
        <StatusLine
          icon={Sparkles}
          label="Fallback behavior"
          value={
            live
              ? "Chart and metrics are backed by persisted eval samples."
              : "The page shows a labeled reference replay until live samples publish."
          }
        />
        <div className="border border-border-default bg-bg px-3 py-3">
          <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
            Why this is not empty
          </p>
          <p className="mt-2 text-sm leading-relaxed text-text-lo">
            A public trust page should explain the evidence contract even when
            the live endpoint is disabled or waiting for its first run. The
            reference replay uses the same three labels and weekly sample shape
            as the persisted harness, without presenting itself as production
            metrics.
          </p>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function Metric({
  icon: Icon,
  label,
  tone = "default",
  value,
}: {
  icon: LucideIcon;
  label: string;
  tone?: "default" | "pnl" | "agent" | "warn" | "risk";
  value: string;
}) {
  return (
    <div className="min-h-24 border-b border-r border-border-default px-4 py-4 last:border-r-0 md:border-b-0">
      <div className="flex items-center gap-2">
        <Icon className={`h-4 w-4 ${toneText(tone)}`} />
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p className={`mt-3 font-mono text-xl font-semibold ${toneText(tone)}`}>
        {value}
      </p>
    </div>
  );
}

function StatusLine({
  icon: Icon,
  label,
  tone = "agent",
  value,
}: {
  icon: LucideIcon;
  label: string;
  tone?: "pnl" | "agent" | "warn" | "risk";
  value: string;
}) {
  return (
    <div className="grid gap-2 border border-border-default bg-bg px-3 py-3">
      <div className="flex items-center gap-2">
        <Icon className={`h-4 w-4 ${toneText(tone)}`} />
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p className="font-mono text-sm text-text-hi">{value}</p>
    </div>
  );
}

function MethodStep({
  body,
  step,
  title,
}: {
  body: string;
  step: string;
  title: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-3 py-3">
      <div className="flex items-center gap-2">
        <span className="flex h-6 w-6 items-center justify-center border border-accent-agent/40 bg-accent-agent/5 font-mono text-xs text-accent-agent">
          {step}
        </span>
        <p className="font-mono text-sm font-semibold text-text-hi">{title}</p>
      </div>
      <p className="mt-3 text-sm leading-relaxed text-text-lo">{body}</p>
    </div>
  );
}

function sourceLabel(status: FetchState["status"]): string {
  if (status === "live") return "Persisted public samples";
  if (status === "empty") return "Endpoint ready, no run published";
  if (status === "disabled") return "Endpoint disabled by feature flag";
  return "Endpoint unreachable during render";
}

function regimeTone(label: Regime): PillTone {
  if (label === "risk_on") return "agent";
  if (label === "risk_off") return "risk";
  return "neutral";
}

function toneText(tone: "default" | "pnl" | "agent" | "warn" | "risk") {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "warn") return "text-warn";
  if (tone === "risk") return "text-risk";
  return "text-text-hi";
}

function formatPct(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}
