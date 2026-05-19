import type { Metadata } from "next";
import Link from "next/link";

import { BrutalCard, BrutalCardBody, BrutalPill, ModelBadge } from "@aegis/ui";
import type { DiaryEntry } from "@/types";

interface PageProps {
  params: Promise<{ wallet: string }>;
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

async function fetchDiary(wallet: string): Promise<DiaryEntry[]> {
  try {
    const res = await fetch(
      `${API_BASE}/diary/wallet/${wallet.toLowerCase()}`,
      {
        next: { revalidate: 60 },
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
  const title = `Aegis agent diary · ${wallet.slice(0, 10)}…`;
  const description =
    "Public, append-only log of every decision the Aegis agent made for this wallet. Model, regime, confidence, and 24h outcome — all on the record.";
  return {
    title,
    description,
    openGraph: { title, description, type: "article" },
    twitter: { card: "summary_large_image", title, description },
  };
}

export default async function DiaryPage({ params }: PageProps) {
  const { wallet } = await params;
  const entries = await fetchDiary(wallet);

  return (
    <main className="min-h-screen bg-[#0A0A0A] text-text-hi px-6 py-12">
      <div className="max-w-3xl mx-auto">
        <header className="mb-10">
          <p className="font-mono text-[11px] tracking-wider text-accent-agent uppercase">
            Agent diary
          </p>
          <h1 className="text-3xl font-bold mt-1">
            <span className="font-mono text-base text-text-lo">{wallet}</span>
          </h1>
          <p className="text-sm text-text-lo mt-3 max-w-prose">
            Every recommendation Aegis emitted for this wallet, the model that
            produced it, the regime read at the time, and what actually happened
            to the portfolio over the next 24 hours.
          </p>
        </header>

        {entries.length === 0 ? (
          <BrutalCard>
            <BrutalCardBody>
              <p className="text-text-lo text-sm">
                No public decisions yet. The portfolio owner can enable diary
                visibility from their settings.
              </p>
            </BrutalCardBody>
          </BrutalCard>
        ) : (
          <ol className="space-y-4">
            {entries.map((entry) => (
              <DiaryCard key={entry.decisionId} entry={entry} />
            ))}
          </ol>
        )}

        <footer className="mt-12 pt-6 border-t border-white/10 text-xs text-text-mut">
          <Link href="/" className="hover:text-accent-agent">
            ← back to aegis
          </Link>
        </footer>
      </div>
    </main>
  );
}

function DiaryCard({ entry }: { entry: DiaryEntry }) {
  return (
    <li>
      <BrutalCard>
        <BrutalCardBody>
          <div className="flex items-start justify-between gap-4 mb-3">
            <div className="flex items-center gap-2">
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
                <span className="text-xs px-2 py-0.5 rounded bg-white/10 text-text-hi/80 border border-white/20">
                  Critic:{" "}
                  {entry.criticVerdict.verdict === "revised"
                    ? "revised"
                    : "approved"}
                </span>
              )}
            </div>
            <time
              className="font-mono text-[11px] text-text-mut"
              dateTime={entry.createdAt}
            >
              {new Date(entry.createdAt).toLocaleString()}
            </time>
          </div>
          <p className="text-sm text-text-hi">{entry.recommendationSummary}</p>
          <div className="mt-2 h-1.5 bg-white/5">
            <div
              className="h-full bg-cyan-400"
              style={{ width: `${Math.round(entry.confidence * 100)}%` }}
            />
          </div>
          <p className="mt-1 font-mono text-[11px] text-text-mut">
            confidence {Math.round(entry.confidence * 100)}%
          </p>
          {entry.outcome ? (
            <div className="mt-4 border-t border-white/5 pt-3 grid grid-cols-2 gap-4 text-xs font-mono">
              <div>
                <div className="text-text-mut">Realized 24h</div>
                <div
                  className={
                    entry.outcome.realizedPctChange >= 0
                      ? "text-accent-pnl"
                      : "text-risk"
                  }
                >
                  {entry.outcome.realizedPctChange >= 0 ? "+" : ""}
                  {entry.outcome.realizedPctChange.toFixed(2)}%
                </div>
              </div>
              <div>
                <div className="text-text-mut">Counterfactual</div>
                <div
                  className={
                    entry.outcome.counterfactualPctChange >= 0
                      ? "text-accent-pnl"
                      : "text-risk"
                  }
                >
                  {entry.outcome.counterfactualPctChange >= 0 ? "+" : ""}
                  {entry.outcome.counterfactualPctChange.toFixed(2)}%
                </div>
              </div>
            </div>
          ) : (
            <p className="mt-4 text-[11px] text-text-mut">
              Outcome will be recorded ~24h after the decision.
            </p>
          )}
        </BrutalCardBody>
      </BrutalCard>
    </li>
  );
}
