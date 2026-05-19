import type { Metadata } from "next";
import Link from "next/link";
import { Trophy } from "lucide-react";
import { ModelBadge } from "@aegis/ui";

export const metadata: Metadata = {
  title: "Aegis · Leaderboard",
  description:
    "How the Aegis agent has performed across all live portfolios — ranked by realized 7d return vs counterfactual.",
};

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
  excellent: "text-accent-pnl border-accent-pnl/30 bg-accent-pnl/5",
  strong: "text-accent-pnl/80 border-accent-pnl/20 bg-accent-pnl/5",
  stable: "text-text-default border-border-default bg-raised",
  shaky: "text-amber-300 border-amber-500/30 bg-amber-500/5",
  underperforming: "text-risk border-risk/30 bg-risk/5",
};

async function fetchLeaderboard(): Promise<LeaderboardEntry[]> {
  const apiBase = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
  try {
    const res = await fetch(`${apiBase}/leaderboard?limit=50`, {
      cache: "no-store",
    });
    if (!res.ok) return [];
    return (await res.json()) as LeaderboardEntry[];
  } catch {
    return [];
  }
}

export default async function LeaderboardPage() {
  const rows = await fetchLeaderboard();

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div className="flex items-center gap-3">
        <Trophy className="w-5 h-5 text-accent-pnl" />
        <div>
          <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
            Leaderboard
          </h1>
          <p className="text-sm text-text-lo mt-0.5">
            Ranked by 7d realized return vs the agent&apos;s own counterfactual.
            Handles are anonymous hashes — no wallet addresses are exposed.
          </p>
        </div>
      </div>

      {rows.length === 0 ? (
        <EmptyState />
      ) : (
        <div className="border-brutal border-border-default">
          <TableHeader />
          <ul>
            {rows.map((row, i) => (
              <Row key={row.userId} entry={row} rank={i + 1} />
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function TableHeader() {
  return (
    <div className="grid grid-cols-[40px_1fr_80px_80px_80px_100px] gap-3 px-4 py-3 border-b border-border-default text-[10px] font-mono uppercase tracking-wider text-text-lo bg-raised">
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
      ? "text-accent-pnl"
      : entry.trustabilityDelta < 0
        ? "text-risk"
        : "text-text-default";
  const labelTone = LABEL_TONE[entry.label];

  return (
    <li className="grid grid-cols-[40px_1fr_80px_80px_80px_100px] gap-3 items-center px-4 py-3 border-b border-border-default last:border-0 font-mono text-xs hover:bg-white/2 transition-colors">
      <span className="text-text-mut">{rank.toString().padStart(2, "0")}</span>
      <Link
        href={`/diary/${entry.handle}`}
        className="text-text-hi hover:text-accent-agent transition-colors inline-flex items-center gap-2"
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
                ? "border-risk/40 text-risk"
                : "border-accent-agent/40 text-accent-agent"
            }`}
          >
            {entry.recentCriticVerdict.verdict ??
              (entry.recentCriticVerdict.demandsRevision
                ? "revised"
                : "approved")}
          </span>
        )}
        {entry.distinctModels > 1 && (
          <span className="ml-1 text-[10px] text-accent-agent/70">
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
    <div className="border-brutal border-border-default bg-raised p-12 text-center space-y-3">
      <p className="text-sm font-mono text-text-lo">No live portfolios yet.</p>
      <p className="text-xs font-mono text-text-mut">
        The leaderboard fills out as the agent has 24h of outcomes to compare
        against its own counterfactuals.
      </p>
    </div>
  );
}
