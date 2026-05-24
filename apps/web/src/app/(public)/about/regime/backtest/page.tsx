import type { Metadata } from "next";
import Link from "next/link";
import { Activity, ArrowRight } from "lucide-react";
import { BrutalPill } from "@aegis/ui";
import { LandingShell } from "@/components/layout/landing-shell";
import type { Sample } from "@/components/regime/backtest-chart";
import { pageMetadata } from "@/lib/seo";
import {
  buildReferenceSamples,
  REFERENCE_MODEL_SLUG,
  REFERENCE_RUN_ID,
  REGIME_ORDER,
  type Regime,
} from "./backtest-reference";
import {
  EvidenceGrid,
  EventList,
  MethodologyPanel,
  MetricStrip,
  PerRegimeTable,
  type FetchState,
  type RegimeSummary,
  type Summary,
} from "./backtest-page-sections";

export const metadata: Metadata = pageMetadata({
  title: "Regime Classifier Backtest Evidence — Aegis",
  description:
    "Backtest evidence for the Aegis market-regime classifier. Out-of-sample predictions replayed across historical price data.",
  path: "/about/regime/backtest",
});

export const dynamic = "force-dynamic";
export const revalidate = 0;

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

interface SamplesResponse {
  evalRunId: string | null;
  modelSlug: string | null;
  samples: Sample[];
}

async function fetchSamples(): Promise<FetchState> {
  try {
    const res = await fetch(
      `${API_BASE}/about/regime/backtest/samples?limit=1200`,
      { cache: "no-store" },
    );
    if (res.status === 404) return emptyFetch("disabled");
    if (!res.ok) return emptyFetch("unreachable");

    const body = (await res.json()) as SamplesResponse;
    if (!body.samples.length) {
      return {
        ...emptyFetch("empty"),
        evalRunId: body.evalRunId,
        modelSlug: body.modelSlug,
      };
    }
    return {
      evalRunId: body.evalRunId,
      modelSlug: body.modelSlug,
      samples: body.samples,
      status: "live",
    };
  } catch {
    return emptyFetch("unreachable");
  }
}

export default async function RegimeBacktestPage() {
  const fetched = await fetchSamples();
  const samples = fetched.samples.length
    ? fetched.samples
    : buildReferenceSamples();
  const summary = summarize(samples);
  const live = fetched.status === "live";
  const modelSlug = fetched.modelSlug ?? REFERENCE_MODEL_SLUG;
  const runId = fetched.evalRunId ?? REFERENCE_RUN_ID;

  return (
    <LandingShell width="wide">
      <header className="mb-6 grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
        <div className="min-w-0">
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <Activity className="h-5 w-5 text-accent-agent" />
            <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
              Regime classifier · backtest
            </p>
            <BrutalPill tone={live ? "pnl" : "warn"}>
              {live ? "LIVE RUN" : "REFERENCE REPLAY"}
            </BrutalPill>
          </div>
          <h1 className="font-mono text-3xl font-semibold tracking-tight text-text-hi md:text-4xl">
            Backtest evidence that can be inspected
          </h1>
          <p className="mt-3 max-w-3xl text-sm leading-relaxed text-text-lo">
            Weekly out-of-sample regime calls are replayed against historical
            price windows. The classifier only sees backward-looking volatility,
            drawdown, and correlation features; realized labels come from the
            following 30-day BTC return bucket.
          </p>
        </div>
        <Link
          href="/about/regime"
          className="inline-flex min-h-10 items-center justify-center gap-2 border border-accent-agent/40 bg-accent-agent/5 px-4 font-mono text-xs font-semibold text-accent-agent hover:border-accent-agent"
        >
          Open model card
          <ArrowRight className="h-3.5 w-3.5" />
        </Link>
      </header>

      <MetricStrip summary={summary} live={live} />

      <EvidenceGrid
        fetched={fetched}
        live={live}
        modelSlug={modelSlug}
        runId={runId}
        samples={samples}
      />

      <section className="mt-5 grid gap-5 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
        <PerRegimeTable rows={summary.perRegime} />
        <EventList />
      </section>

      <MethodologyPanel />
    </LandingShell>
  );
}

function summarize(samples: Sample[]): Summary {
  const normalized = samples.map((sample) => ({
    observedAt: sample.observedAt,
    predicted: normalizeRegime(sample.predictedLabel),
    realized: normalizeRegime(sample.realizedLabel),
  }));
  const correct = normalized.filter(
    (sample) => sample.predicted === sample.realized,
  ).length;
  const transitions = normalized.filter((sample, index) => {
    return index > 0 && sample.predicted !== normalized[index - 1]?.predicted;
  }).length;
  const perRegime = REGIME_ORDER.map((label) =>
    summarizeRegime(normalized, label),
  );
  const start = formatDate(normalized[0]?.observedAt);
  const end = formatDate(normalized[normalized.length - 1]?.observedAt);

  return {
    agreement: correct / Math.max(normalized.length, 1),
    end,
    perRegime,
    samplesCount: normalized.length,
    start,
    transitions,
  };
}

function summarizeRegime(
  samples: Array<{ predicted: Regime; realized: Regime }>,
  label: Regime,
): RegimeSummary {
  const predicted = samples.filter(
    (sample) => sample.predicted === label,
  ).length;
  const actual = samples.filter((sample) => sample.realized === label).length;
  const truePositive = samples.filter(
    (sample) => sample.predicted === label && sample.realized === label,
  ).length;
  const precision = truePositive / Math.max(predicted, 1);
  const recall = truePositive / Math.max(actual, 1);
  const f1 =
    precision + recall > 0
      ? (2 * precision * recall) / (precision + recall)
      : 0;

  return { actual, f1, label, precision, predicted, recall };
}

function normalizeRegime(value: string): Regime {
  if (value === "risk_on" || value === "risk_off" || value === "neutral") {
    return value;
  }
  return "neutral";
}

function emptyFetch(status: FetchState["status"]): FetchState {
  return { evalRunId: null, modelSlug: null, samples: [], status };
}

function formatDate(value: string | undefined): string {
  if (!value) return "—";
  return new Date(value).toISOString().slice(0, 10);
}
