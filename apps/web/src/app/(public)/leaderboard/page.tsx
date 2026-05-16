import type { Metadata } from "next";
import Link from "next/link";
import { Trophy } from "lucide-react";
import { ModelBadge } from "@aegis/ui";

export const metadata: Metadata = {
  title: "Aegis · Leaderboard",
  description:
    "How the Aegis agent has performed across all live portfolios — ranked by realized 7d return vs counterfactual.",
};

// Rendered at request time so the score reflects the latest decisions.
// SSE will pick up `leaderboard.update` for in-page refresh.
export const dynamic = "force-dynamic";
export const revalidate = 0;

interface LeaderboardEntry {
  userId: string;
  handle: string;
  decisionsExecuted: number;
  distinctModels: number;
  avg7dReturn: number;
  trustabilityDelta: number;
  lastDecisionAt: string | null;
  label: "excellent" | "strong" | "stable" | "shaky" | "underperforming";
  recentModelSlug?: string;
  recentCriticVerdict?: {
    verdict?: "approved" | "revised" | "abstained";
    demandsRevision?: boolean;
  };
}

const LABEL_TONE: Record<LeaderboardEntry["label"], string> = {
  excellent: "text-emerald-300 border-emerald-500/30 bg-emerald-500/5",
  strong: "text-emerald-200/90 border-emerald-500/20 bg-emerald-500/5",
  stable: "text-white border-white/15 bg-white/3",
  shaky: "text-amber-300 border-amber-500/30 bg-amber-500/5",
  underperforming: "text-rose-300 border-rose-500/30 bg-rose-500/5",
};

async function fetchLeaderboard(): Promise<LeaderboardEntry[]> {
  const apiBase = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
  try {
    const res = await fetch(`${apiBase}/leaderboard?limit=50`, {
      // No auth — the endpoint is public.
      cache: "no-store",
    });
    if (!res.ok) return [];
    return (await res.json()) as LeaderboardEntry[];
  } catch {
    // Render the empty state rather than crashing the page if the API is
    // unreachable — the page is publicly shared and shouldn't 500.
    return [];
  }
}

export default async function LeaderboardPage() {
  const rows = await fetchLeaderboard();

  return (
    <main className="min-h-screen bg-bg text-text-default px-6 py-10">
      <div className="max-w-4xl mx-auto">
        <header className="flex items-center gap-3 mb-6">
          <Trophy className="w-6 h-6 text-accent-pnl" />
          <div>
            <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
              Leaderboard
            </h1>
            <p className="text-sm text-text-lo mt-0.5">
              Ranked by 7d realized return vs the agent&apos;s own
              counterfactual. Handles are anonymous hashes — no wallet addresses
              are exposed.
            </p>
          </div>
        </header>

        {rows.length === 0 ? (
          <EmptyState />
        ) : (
          <div className="border-2 border-white/10 bg-[#141414]">
            <Header />
            <ul>
              {rows.map((row, i) => (
                <Row key={row.userId} entry={row} rank={i + 1} />
              ))}
            </ul>
          </div>
        )}

        <p className="mt-6 text-center text-xs font-mono text-text-mut">
          Just looking?{" "}
          <Link href="/explore" className="text-accent-agent hover:underline">
            Explore demo portfolios
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

function Header() {
  return (
    <div className="grid grid-cols-[40px_1fr_80px_80px_80px_100px] gap-3 px-4 py-3 border-b-2 border-white/10 text-[10px] font-mono uppercase tracking-wider text-text-lo">
      <span>#</span>
      <span>Handle</span>
      <span className="text-right">Δ vs cf</span>
      <span className="text-right">7d avg</span>
      <span className="text-right">Decisions</span>
      <span className="text-right">Tier</span>
    </div>
  );
}

function Row({ entry, rank }: { entry: LeaderboardEntry; rank: number }) {
  const deltaSign = entry.trustabilityDelta > 0 ? "+" : "";
  const deltaTone =
    entry.trustabilityDelta > 0
      ? "text-emerald-300"
      : entry.trustabilityDelta < 0
        ? "text-rose-300"
        : "text-text-default";
  const labelTone = LABEL_TONE[entry.label];

  return (
    <li className="grid grid-cols-[40px_1fr_80px_80px_80px_100px] gap-3 items-center px-4 py-3 border-b border-white/4 last:border-0 font-mono text-xs hover:bg-white/2 transition-colors">
      <span className="text-text-mut">{rank.toString().padStart(2, "0")}</span>
      <Link
        href={`/diary/${entry.handle}`}
        className="text-text-hi hover:text-cyan-300 transition-colors inline-flex items-center gap-2"
      >
        <span>
          <span className="opacity-70">0x</span>
          {entry.handle}
        </span>
        {entry.recentModelSlug && <ModelBadge model={entry.recentModelSlug} />}
        {entry.recentCriticVerdict && (
          <span
            className={`text-[9px] px-1 py-0.5 font-mono border ${
              entry.recentCriticVerdict.verdict === "revised" ||
              entry.recentCriticVerdict.demandsRevision
                ? "border-rose-500/40 text-rose-300"
                : "border-cyan-500/40 text-cyan-300"
            }`}
          >
            {entry.recentCriticVerdict.verdict ??
              (entry.recentCriticVerdict.demandsRevision
                ? "revised"
                : "approved")}
          </span>
        )}
        {entry.distinctModels > 1 && (
          <span className="ml-1 text-[10px] text-cyan-300/70">
            {entry.distinctModels} models
          </span>
        )}
      </Link>
      <span className={`text-right tabular-nums ${deltaTone}`}>
        {deltaSign}
        {entry.trustabilityDelta.toFixed(2)}%
      </span>
      <span className="text-right tabular-nums text-text-default">
        {entry.avg7dReturn >= 0 ? "+" : ""}
        {entry.avg7dReturn.toFixed(2)}%
      </span>
      <span className="text-right tabular-nums text-text-lo">
        {entry.decisionsExecuted}
      </span>
      <span
        className={`text-right text-[10px] uppercase tracking-wider border px-1.5 py-0.5 ${labelTone}`}
      >
        {entry.label}
      </span>
    </li>
  );
}

function EmptyState() {
  return (
    <div className="border-2 border-dashed border-white/10 bg-[#141414] p-12 text-center">
      <p className="text-sm text-text-default mb-2">No live portfolios yet.</p>
      <p className="text-xs text-text-lo mb-6">
        The leaderboard fills out as the agent has 24h of outcomes to compare
        against its own counterfactuals.
      </p>
      <Link
        href="/signup"
        className="inline-block px-4 py-2 text-sm font-semibold border-2 border-emerald-300 bg-emerald-500 text-black hover:bg-emerald-400 transition-colors"
      >
        Be the first
      </Link>
    </div>
  );
}
