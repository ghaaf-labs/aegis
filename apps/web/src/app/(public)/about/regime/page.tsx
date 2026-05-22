import type { Metadata } from "next";
import Link from "next/link";
import { Activity } from "lucide-react";
import { LandingShell } from "@/components/layout/landing-shell";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
  ModelBadge,
  ProvenanceLine,
} from "@aegis/ui";

export const metadata: Metadata = {
  title: "Aegis · Regime classifier — model card",
  description:
    "Backtest precision/recall numbers for the Aegis market-regime classifier. Trust signal, not marketing.",
};

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
    <LandingShell>
      <header className="flex items-center gap-3 mb-6">
        <Activity className="w-6 h-6 text-accent-agent" />
        <div>
          <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
            Regime classifier — model card
          </h1>
          <p className="text-sm text-text-lo mt-0.5 max-w-2xl">
            How well does the Aegis regime classifier actually call the market?
            Numbers below are from a replay backtest of the live model against
            historical price data — no marketing gloss, no hand-picked windows.
          </p>
        </div>
      </header>

      {evaluation ? (
        <div className="space-y-6">
          <SummaryCard evaluation={evaluation} />
          <PerRegimeTable perRegime={evaluation.perRegimeJsonb} />
          <ConfusionMatrix confusion={evaluation.confusionJsonb} />
          <MethodologyNote />
          <p className="text-[10px] text-text-mut font-mono">
            Page rendered at {pageRenderedAt}. Backtest run id{" "}
            <span className="text-accent-agent">{evaluation.evalRunId}</span>.
          </p>
        </div>
      ) : (
        <EmptyState />
      )}

      <p className="mt-8 text-center text-xs font-mono text-text-mut">
        Want to see it on a live portfolio?{" "}
        <Link
          href="/explore"
          className="inline-flex min-h-9 items-center text-accent-agent hover:underline"
        >
          Explore demo portfolios
        </Link>{" "}
        ·{" "}
        <Link
          href="/login"
          className="inline-flex min-h-9 items-center text-accent-pnl hover:underline"
        >
          Create your own
        </Link>
      </p>
    </LandingShell>
  );
}

function SummaryCard({ evaluation }: { evaluation: EvaluationRow }) {
  return (
    <BrutalCard variant="raised">
      <BrutalCardHeader>
        <div className="flex items-center gap-3">
          <span className="text-xs uppercase tracking-wider text-text-lo font-mono">
            Latest evaluation
          </span>
          <ModelBadge model={evaluation.modelSlug} />
        </div>
        <ProvenanceLine
          source="Aegis backtest harness"
          freshness={`${evaluation.samplesCount} samples`}
        />
      </BrutalCardHeader>
      <BrutalCardBody>
        <dl className="grid grid-cols-2 sm:grid-cols-4 gap-4 font-mono">
          <Metric label="Accuracy" value={formatPct(evaluation.accuracy)} />
          <Metric
            label="Precision (macro)"
            value={formatPct(evaluation.precisionMacro)}
          />
          <Metric
            label="Recall (macro)"
            value={formatPct(evaluation.recallMacro)}
          />
          <Metric label="F1 (macro)" value={formatPct(evaluation.f1Macro)} />
          <Metric
            label="Brier score"
            value={formatNumber(evaluation.brierScore)}
            help="lower is better"
          />
          <Metric
            label="Period"
            value={`${evaluation.periodStart} → ${evaluation.periodEnd}`}
            wide
          />
        </dl>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function PerRegimeTable({
  perRegime,
}: {
  perRegime: Record<Regime, PerRegime>;
}) {
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-xs uppercase tracking-wider text-text-lo font-mono">
          Per-regime breakdown
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="p-0">
        <table className="w-full font-mono text-xs">
          <thead>
            <tr className="text-[10px] uppercase tracking-wider text-text-mut border-b border-border-default">
              <th className="text-left px-4 py-2">Regime</th>
              <th className="text-right px-4 py-2">Precision</th>
              <th className="text-right px-4 py-2">Recall</th>
              <th className="text-right px-4 py-2">F1</th>
              <th className="text-right px-4 py-2">Support</th>
            </tr>
          </thead>
          <tbody>
            {REGIME_ORDER.map((r) => {
              const m = perRegime?.[r];
              if (!m) return null;
              return (
                <tr
                  key={r}
                  className="border-b border-border-default/40 last:border-0"
                >
                  <td className="px-4 py-2">
                    <BrutalPill tone="neutral">{REGIME_LABEL[r]}</BrutalPill>
                  </td>
                  <td className="text-right px-4 py-2 tabular-nums">
                    {formatPct(m.precision)}
                  </td>
                  <td className="text-right px-4 py-2 tabular-nums">
                    {formatPct(m.recall)}
                  </td>
                  <td className="text-right px-4 py-2 tabular-nums">
                    {formatPct(m.f1)}
                  </td>
                  <td className="text-right px-4 py-2 tabular-nums text-text-lo">
                    {m.support}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function ConfusionMatrix({ confusion }: { confusion: Confusion }) {
  const labels = confusion.labels ?? REGIME_ORDER;
  const rows = confusion.rows ?? [];

  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-xs uppercase tracking-wider text-text-lo font-mono">
          Confusion matrix
        </span>
        <span className="text-[10px] text-text-mut font-mono">
          rows = actual · columns = predicted
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="p-0 overflow-x-auto">
        <table className="w-full font-mono text-xs">
          <thead>
            <tr className="text-[10px] uppercase tracking-wider text-text-mut border-b border-border-default">
              <th className="text-left px-4 py-2"> </th>
              {labels.map((c) => (
                <th key={`col-${c}`} className="text-right px-4 py-2">
                  Predicted {REGIME_LABEL[c]}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, ri) => {
              const label = labels[ri] ?? REGIME_ORDER[ri] ?? "neutral";
              return (
                <tr
                  key={`row-${label}`}
                  className="border-b border-border-default/40 last:border-0"
                >
                  <td className="px-4 py-2 text-text-hi">
                    Actual {REGIME_LABEL[label]}
                  </td>
                  {row.map((cell, ci) => {
                    const onDiagonal = ri === ci;
                    return (
                      <td
                        key={`cell-${ri}-${ci}`}
                        className={`text-right px-4 py-2 tabular-nums ${
                          onDiagonal
                            ? "text-text-hi font-semibold"
                            : "text-text-lo"
                        }`}
                      >
                        {cell}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function MethodologyNote() {
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-xs uppercase tracking-wider text-text-lo font-mono">
          How &ldquo;realized&rdquo; is labeled
        </span>
      </BrutalCardHeader>
      <BrutalCardBody>
        <p className="text-sm text-text-default mb-3">
          We label each historical window&apos;s &ldquo;true&rdquo; regime from
          its 30-day forward BTC return:
        </p>
        <ul className="text-sm text-text-default space-y-1 list-disc pl-6 mb-3">
          <li>
            <span className="font-mono text-accent-agent">RISK-OFF</span> if BTC
            fell more than 10% over the next 30 days.
          </li>
          <li>
            <span className="font-mono text-accent-agent">RISK-ON</span> if BTC
            rose more than 10%.
          </li>
          <li>
            <span className="font-mono text-accent-agent">NEUTRAL</span>{" "}
            otherwise.
          </li>
        </ul>
        <p className="text-sm text-text-lo">
          It&apos;s a crude, defensible heuristic — not perfect, but it
          can&apos;t be cherry-picked. The classifier itself only sees
          backward-looking features (BTC 30d realized vol, 90d cross-asset
          correlation, 30d drawdown) computed from the same{" "}
          <span className="font-mono">price_history</span> rows that power the
          live agent. The numbers above will change as more decisions land and
          the next backtest run lands.
        </p>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function EmptyState() {
  return (
    <BrutalCard>
      <BrutalCardBody className="text-center py-12">
        <p className="text-sm text-text-default mb-2">
          Evaluation data not yet available.
        </p>
        <p className="text-xs text-text-lo">
          The model card populates after the first live evaluation cycle
          completes. Check back once the agent has processed enough decisions.
        </p>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function Metric({
  label,
  value,
  help,
  wide,
}: {
  label: string;
  value: string;
  help?: string;
  wide?: boolean;
}) {
  return (
    <div className={wide ? "col-span-2" : ""}>
      <dt className="text-[10px] uppercase tracking-wider text-text-mut">
        {label}
      </dt>
      <dd className="text-base text-text-hi tabular-nums mt-1">{value}</dd>
      {help && <p className="text-[10px] text-text-mut mt-0.5">{help}</p>}
    </div>
  );
}

function formatPct(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return "—";
  return `${(v * 100).toFixed(1)}%`;
}

function formatNumber(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return "—";
  return v.toFixed(4);
}
