import type { Metadata } from "next";
import Link from "next/link";
import { ModelBadge } from "@aegis/ui";
import type { DiaryEntry } from "@/types";
import {
  AuditTrail,
  type DecisionFull,
} from "@/components/decision/audit-trail";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
const PUBLIC_BASE = process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:3000";

interface RouteParams {
  params: Promise<{ decisionId: string }>;
}

async function fetchDecision(id: string): Promise<DiaryEntry | null> {
  try {
    const res = await fetch(`${API_BASE}/diary/decision/${id}`, {
      next: { revalidate: 60 },
    });
    if (!res.ok) return null;
    return (await res.json()) as DiaryEntry;
  } catch {
    return null;
  }
}

async function fetchDecisionFull(id: string): Promise<DecisionFull | null> {
  try {
    const res = await fetch(`${API_BASE}/diary/decision/${id}/full`, {
      next: { revalidate: 60 },
    });
    if (!res.ok) return null;
    return (await res.json()) as DecisionFull;
  } catch {
    return null;
  }
}

export async function generateMetadata({
  params,
}: RouteParams): Promise<Metadata> {
  const { decisionId } = await params;
  const decision = await fetchDecision(decisionId);
  const summary = decision?.recommendationSummary ?? "An Aegis agent decision";
  const realized = decision?.outcome?.realizedPctChange;
  const description =
    realized != null
      ? `${realized >= 0 ? "+" : ""}${realized.toFixed(2)}% realized · ${summary}`
      : summary;
  const ogImage = `${PUBLIC_BASE}/og/${decisionId}`;
  return {
    title: `Aegis · ${summary}`,
    description,
    openGraph: {
      title: "Aegis decision",
      description,
      type: "article",
      url: `${PUBLIC_BASE}/decision/${decisionId}`,
      images: [{ url: ogImage, width: 1200, height: 630 }],
    },
    twitter: {
      card: "summary_large_image",
      title: "Aegis decision",
      description,
      images: [ogImage],
    },
  };
}

export default async function DecisionPage({ params }: RouteParams) {
  const { decisionId } = await params;
  const [decision, fullTrail] = await Promise.all([
    fetchDecision(decisionId),
    fetchDecisionFull(decisionId),
  ]);

  if (!decision) {
    return (
      <main className="min-h-screen bg-bg text-text-default flex items-center justify-center px-6">
        <div className="max-w-md text-center">
          <p className="text-sm text-text-default mb-2">Decision not found.</p>
          <p className="text-xs text-text-lo mb-6">
            It may be private — the portfolio owner hasn&apos;t enabled the
            public diary.
          </p>
          <Link
            href="/leaderboard"
            className="text-accent-agent hover:underline"
          >
            See the leaderboard →
          </Link>
        </div>
      </main>
    );
  }

  const realized = decision.outcome?.realizedPctChange;
  const counterfactual = decision.outcome?.counterfactualPctChange;

  return (
    <main className="min-h-screen bg-bg text-text-default px-6 py-10">
      <div className="max-w-2xl mx-auto space-y-6">
        <header className="space-y-2">
          <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
            Aegis decision · {decisionId.slice(0, 8)}…
          </p>
          <h1 className="text-3xl font-semibold text-text-hi font-mono tracking-tight">
            {decision.recommendationSummary}
          </h1>
          <div className="flex flex-wrap items-center gap-2 text-sm text-text-lo">
            {decision.regime && (
              <span className="uppercase tracking-wider">
                {decision.regime.replace("_", " ")} regime
              </span>
            )}
            {decision.modelSlug && <ModelBadge model={decision.modelSlug} />}
            <span>confidence {Math.round(decision.confidence * 100)}%</span>
          </div>

          {decision.criticVerdict && (
            <div className="mt-1 inline-flex items-center gap-2 text-xs">
              <span className="uppercase tracking-[1px] text-text-lo">
                Critic
              </span>
              <span
                className={`px-2 py-0.5 font-mono border ${
                  decision.criticVerdict.verdict === "revised"
                    ? "border-rose-500/40 text-risk bg-rose-500/10"
                    : "border-cyan-500/40 text-accent-agent bg-cyan-500/10"
                }`}
              >
                {decision.criticVerdict.verdict.toUpperCase()}
              </span>
              <span className="text-text-mut max-w-[420px] truncate">
                {decision.criticVerdict.notes}
              </span>
            </div>
          )}
        </header>

        <p className="text-[10px] text-text-mut font-mono">
          Decision recorded {new Date(decision.createdAt).toLocaleString()} ·
          via agent pipeline
        </p>

        {realized != null && counterfactual != null && (
          <section className="grid grid-cols-2 gap-3">
            <Stat
              label="Realized (24h)"
              value={`${realized >= 0 ? "+" : ""}${realized.toFixed(2)}%`}
              tone={realized >= 0 ? "pnl" : "risk"}
            />
            <Stat
              label="Counterfactual"
              value={`${counterfactual >= 0 ? "+" : ""}${counterfactual.toFixed(2)}%`}
              tone="agent"
            />
          </section>
        )}

        {fullTrail && (
          <div className="space-y-2">
            <h2 className="text-xs font-mono uppercase tracking-widest text-text-lo">
              Audit trail
            </h2>
            <AuditTrail data={fullTrail} />
          </div>
        )}

        <section className="border-2 border-white/10 bg-[#141414] p-4">
          <h2 className="text-xs font-mono uppercase tracking-wider text-text-lo mb-2">
            Why the agent acted
          </h2>
          <p className="text-sm text-text-default whitespace-pre-line">
            {decision.outcome?.compressedSummary ??
              "(outcome will land 24h after the decision)"}
          </p>
        </section>

        <p className="text-center text-xs font-mono text-text-mut">
          <Link href="/leaderboard" className="hover:underline">
            See the leaderboard
          </Link>{" "}
          ·{" "}
          <Link href="/explore" className="text-accent-agent hover:underline">
            Explore the agent
          </Link>{" "}
          ·{" "}
          <Link href="/signup" className="text-accent-pnl hover:underline">
            Create your own
          </Link>
        </p>
      </div>
    </main>
  );
}

function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "pnl" | "agent" | "risk";
}) {
  const colors = {
    pnl: "text-accent-pnl border-accent-pnl/30",
    agent: "text-accent-agent border-accent-agent/30",
    risk: "text-risk border-risk/30",
  };
  return (
    <div className={`border-2 ${colors[tone]} bg-[#141414] p-4`}>
      <p className="text-[10px] font-mono uppercase tracking-wider text-text-lo mb-1">
        {label}
      </p>
      <p
        className={`text-2xl font-mono tabular-nums ${colors[tone].split(" ")[0]}`}
      >
        {value}
      </p>
    </div>
  );
}
