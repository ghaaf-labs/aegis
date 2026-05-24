import type { Metadata } from "next";
import { notFound } from "next/navigation";
import Link from "next/link";
import { ArrowRight, BookOpen, ShieldCheck } from "lucide-react";

import { BrutalCard, BrutalCardBody, BrutalPill, ModelBadge } from "@aegis/ui";
import type { DiaryEntry } from "@/types";

interface PageProps {
  params: Promise<{ wallet: string }>;
}

const DIARY_IDENTIFIER_RE =
  /^(0x[0-9a-fA-F]{40}|[0-9a-fA-F]{8}|[0-9a-fA-F]{32})$/;
const WALLET_RE = /^0x[0-9a-fA-F]{40}$/;
const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

async function fetchDiary(identifier: string): Promise<DiaryEntry[]> {
  try {
    const res = await fetch(
      `${API_BASE}/diary/wallet/${identifier.toLowerCase()}`,
      {
        cache: "no-store",
      },
    );
    if (!res.ok) return [];
    return (await res.json()) as DiaryEntry[];
  } catch {
    return [];
  }
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { wallet } = await params;
  if (!DIARY_IDENTIFIER_RE.test(wallet)) {
    return { title: "Diary link invalid — Aegis", robots: { index: false } };
  }
  const isWallet = WALLET_RE.test(wallet);
  const title = `Aegis agent diary · ${isWallet ? wallet.slice(0, 10) : `0x${wallet.slice(0, 8)}`}…`;
  const description =
    "Public opt-in log of Aegis agent decisions, model routing, confidence, critic review, and 24h outcome.";
  return {
    title,
    description,
    openGraph: { title, description, type: "article" },
    twitter: { card: "summary_large_image", title, description },
  };
}

export default async function DiaryPage({ params }: PageProps) {
  const { wallet } = await params;

  if (!DIARY_IDENTIFIER_RE.test(wallet)) {
    notFound();
  }

  const identifier = wallet.toLowerCase();
  const isWallet = WALLET_RE.test(identifier);
  const entries = await fetchDiary(identifier);
  const outcomes = entries.filter((entry) => entry.outcome).length;
  const avgDelta =
    outcomes > 0
      ? entries.reduce((sum, entry) => {
          if (!entry.outcome) return sum;
          return (
            sum +
            entry.outcome.realizedPctChange -
            entry.outcome.counterfactualPctChange
          );
        }, 0) / outcomes
      : 0;

  return (
    <main className="min-h-screen bg-bg px-4 py-8 text-text-hi md:px-6 md:py-12">
      <div className="mx-auto max-w-5xl space-y-5">
        <header className="border-brutal border-border-default bg-surface">
          <div className="grid gap-4 border-b border-border-default px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:px-5">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <BookOpen className="h-4 w-4 text-accent-agent" />
                <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
                  Agent diary
                </p>
              </div>
              <h1 className="mt-2 font-mono text-2xl font-semibold text-text-hi md:text-3xl">
                Public agent diary
              </h1>
              <p className="mt-2 break-all font-mono text-sm text-text-lo">
                {isWallet ? identifier : `0x${identifier}`}
              </p>
            </div>
            <div className="flex items-start gap-2 border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 font-mono text-[11px] text-text-lo">
              <ShieldCheck className="mt-0.5 h-3.5 w-3.5 shrink-0 text-accent-agent" />
              <span>
                {isWallet
                  ? "Visible only while the owner keeps diary sharing on."
                  : "Anonymous handle. Wallet address stays hidden."}
              </span>
            </div>
          </div>

          <div className="grid grid-cols-3 border-b border-border-default">
            <Metric label="Decisions" value={String(entries.length)} />
            <Metric label="Outcomes" value={`${outcomes}/${entries.length}`} />
            <Metric
              label="Avg delta"
              value={outcomes > 0 ? signedPct(avgDelta) : "pending"}
              tone={outcomes > 0 && avgDelta < 0 ? "risk" : "pnl"}
            />
          </div>

          <div className="px-4 py-3 font-mono text-[10px] text-text-mut md:px-5">
            via public diary opt-in · no-store privacy check
          </div>
        </header>

        {entries.length === 0 ? (
          <BrutalCard>
            <BrutalCardBody>
              <p className="font-mono text-sm text-text-lo">
                No public decisions are available for this diary.
              </p>
            </BrutalCardBody>
          </BrutalCard>
        ) : (
          <ol className="space-y-4">
            {entries.map((entry, index) => (
              <DiaryCard
                key={entry.decisionId}
                entry={entry}
                index={entries.length - index}
              />
            ))}
          </ol>
        )}

        <footer className="border-t border-border-default pt-5 font-mono text-xs text-text-mut">
          <Link href="/leaderboard" className="hover:text-accent-agent">
            back to leaderboard
          </Link>
        </footer>
      </div>
    </main>
  );
}

function Metric({
  label,
  tone = "default",
  value,
}: {
  label: string;
  tone?: "default" | "pnl" | "risk";
  value: string;
}) {
  return (
    <div className="min-h-16 border-r border-border-default px-4 py-3 last:border-r-0">
      <p className="truncate font-mono text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={`mt-1 font-mono text-lg font-semibold tabular-nums ${
          tone === "pnl"
            ? "text-accent-pnl"
            : tone === "risk"
              ? "text-risk"
              : "text-text-hi"
        }`}
      >
        {value}
      </p>
    </div>
  );
}

function DiaryCard({ entry, index }: { entry: DiaryEntry; index: number }) {
  const delta = entry.outcome
    ? entry.outcome.realizedPctChange - entry.outcome.counterfactualPctChange
    : null;

  return (
    <li>
      <BrutalCard>
        <BrutalCardBody>
          <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto]">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-mono text-[11px] text-text-mut">
                  #{index.toString().padStart(2, "0")}
                </span>
                {entry.regime && (
                  <BrutalPill
                    tone={
                      entry.regime === "risk_off"
                        ? "risk"
                        : entry.regime === "risk_on"
                          ? "agent"
                          : "neutral"
                    }
                  >
                    {entry.regime.replace("_", " ")}
                  </BrutalPill>
                )}
                {entry.modelSlug && <ModelBadge model={entry.modelSlug} />}
                {entry.criticVerdict && (
                  <span
                    className={`border px-1.5 py-0.5 font-mono text-[10px] uppercase ${
                      entry.criticVerdict.verdict === "revised"
                        ? "border-risk/40 text-risk"
                        : "border-accent-agent/40 text-accent-agent"
                    }`}
                  >
                    {entry.criticVerdict.verdict === "revised"
                      ? "critic revised"
                      : "critic approved"}
                  </span>
                )}
              </div>
              <p className="mt-3 text-sm leading-relaxed text-text-hi">
                {entry.recommendationSummary}
              </p>
            </div>
            <div className="font-mono text-[11px] text-text-mut md:text-right">
              <time dateTime={entry.createdAt}>
                {new Date(entry.createdAt).toLocaleString()}
              </time>
              <Link
                href={`/decision/${entry.decisionId}`}
                className="mt-2 inline-flex items-center gap-1 text-accent-agent hover:underline"
              >
                audit
                <ArrowRight className="h-3 w-3" />
              </Link>
            </div>
          </div>

          <div className="mt-4 h-1.5 border border-border-default bg-bg">
            <div
              className="h-full bg-accent-agent"
              style={{ width: `${Math.round(entry.confidence * 100)}%` }}
            />
          </div>
          <p className="mt-1 font-mono text-[11px] text-text-mut">
            confidence {Math.round(entry.confidence * 100)}%
          </p>

          {entry.outcome ? (
            <div className="mt-4 grid grid-cols-2 gap-2 border-t border-border-default pt-3 font-mono text-xs md:grid-cols-4">
              <OutcomeMetric
                label="Realized"
                value={signedPct(entry.outcome.realizedPctChange)}
                positive={entry.outcome.realizedPctChange >= 0}
              />
              <OutcomeMetric
                label="Counterfactual"
                value={signedPct(entry.outcome.counterfactualPctChange)}
                positive={entry.outcome.counterfactualPctChange >= 0}
              />
              <OutcomeMetric
                label="Delta"
                value={delta === null ? "pending" : signedPct(delta)}
                positive={delta === null || delta >= 0}
              />
              <OutcomeMetric
                label="vs BTC"
                value={
                  entry.outcome.outperformanceVsBtc === undefined
                    ? "n/a"
                    : `${entry.outcome.outperformanceVsBtc >= 0 ? "+" : ""}${entry.outcome.outperformanceVsBtc.toFixed(2)} pts`
                }
                positive={
                  entry.outcome.outperformanceVsBtc === undefined ||
                  entry.outcome.outperformanceVsBtc >= 0
                }
              />
            </div>
          ) : (
            <p className="mt-4 border-t border-border-default pt-3 font-mono text-[11px] text-text-mut">
              Outcome pending.
            </p>
          )}
        </BrutalCardBody>
      </BrutalCard>
    </li>
  );
}

function OutcomeMetric({
  label,
  positive,
  value,
}: {
  label: string;
  positive: boolean;
  value: string;
}) {
  return (
    <div className="border border-border-default bg-bg/70 px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={`mt-1 tabular-nums ${positive ? "text-accent-pnl" : "text-risk"}`}
      >
        {value}
      </p>
    </div>
  );
}

function signedPct(value: number) {
  const rounded = Math.round(value * 100) / 100;
  return `${rounded >= 0 ? "+" : ""}${rounded.toFixed(2)}%`;
}
