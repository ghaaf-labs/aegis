import type { Metadata } from "next";
import Link from "next/link";
import { Trophy } from "lucide-react";
import { ModelBadge } from "@aegis/ui";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata({
  title: "Leaderboard — Aegis",
  description:
    "How the Aegis agent has performed across all live portfolios — ranked by realized 7d return vs counterfactual.",
  path: "/leaderboard",
});

export const dynamic = "force-dynamic";
export const revalidate = 0;

interface LeaderboardEntry {
  userId: string;
  handle: string;
  decisionsExecuted: number;
  decisionsPerWeek: number;
  distinctModels: number;
  avg7dReturn: number;
  trustabilityDelta: number;
  lastDecisionAt: string | null;
  aumUsd: number;
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
  shaky: "text-warn border-amber-500/30 bg-amber-500/5",
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

const GRID_COLS =
  "md:grid-cols-[40px_minmax(0,1fr)_80px_80px_90px_80px_64px_100px]";

function TableHeader() {
  return (
    <div
      className={`hidden md:grid ${GRID_COLS} gap-3 px-4 py-3 border-b border-border-default text-[10px] font-mono uppercase tracking-wider text-text-lo bg-raised`}
    >
      <span>#</span>
      <span>Handle</span>
      <span className="text-right">Δ vs cf</span>
      <span className="text-right">7d avg</span>
      <span className="text-right">AUM</span>
      <span className="text-right">Decisions</span>
      <span className="text-right">/wk</span>
      <span className="text-right">Tier</span>
    </div>
  );
}

function Row({ entry, rank }: { entry: LeaderboardEntry; rank: number }) {
  const deltaRounded = roundPct(entry.trustabilityDelta);
  const deltaTone =
    deltaRounded > 0
      ? "text-accent-pnl"
      : deltaRounded < 0
        ? "text-risk"
        : "text-text-default";
  const labelTone = LABEL_TONE[entry.label];

  return (
    <li
      className={`border-b border-border-default px-4 py-4 font-mono text-xs transition-colors last:border-0 hover:bg-white/2 md:grid ${GRID_COLS} md:items-center md:gap-3 md:py-3`}
    >
      <div className="space-y-3 md:hidden">
        <div className="flex min-w-0 items-start gap-3">
          <span
            aria-label={`Rank ${rank}`}
            className="mt-0.5 shrink-0 text-text-mut"
          >
            {rank.toString().padStart(2, "0")}
          </span>
          <LeaderboardIdentity entry={entry} />
        </div>
        <div className="grid grid-cols-2 gap-2">
          <MobileMetric
            label="Delta"
            value={signedPct(entry.trustabilityDelta)}
            className={deltaTone}
          />
          <MobileMetric label="7d avg" value={signedPct(entry.avg7dReturn)} />
          <MobileMetric
            label="AUM"
            value={formatAum(entry.aumUsd)}
            className="text-accent-pnl"
          />
          <MobileMetric
            label="Decisions / wk"
            value={`${entry.decisionsExecuted} · ${entry.decisionsPerWeek}/wk`}
            className="text-accent-agent"
          />
          <div className="border border-border-default bg-bg px-3 py-2">
            <p className="text-[10px] uppercase tracking-widest text-text-mut">
              Tier
            </p>
            <span
              className={`mt-1 inline-flex max-w-full items-center border px-1.5 py-0.5 text-[10px] uppercase tracking-wider ${labelTone}`}
            >
              {entry.label}
            </span>
          </div>
        </div>
      </div>

      <span className="hidden text-text-mut md:block">
        {rank.toString().padStart(2, "0")}
      </span>
      <div className="hidden min-w-0 md:block">
        <LeaderboardIdentity entry={entry} />
      </div>
      <span className={`hidden text-right tabular-nums md:block ${deltaTone}`}>
        {signedPct(entry.trustabilityDelta)}
      </span>
      <span className="hidden text-right tabular-nums text-text-default md:block">
        {signedPct(entry.avg7dReturn)}
      </span>
      <span className="hidden text-right tabular-nums text-accent-pnl md:block">
        {formatAum(entry.aumUsd)}
      </span>
      <span className="hidden text-right tabular-nums text-text-lo md:block">
        {entry.decisionsExecuted}
      </span>
      <span className="hidden text-right tabular-nums text-accent-agent md:block">
        {entry.decisionsPerWeek}
      </span>
      <span
        className={`hidden text-right text-[10px] uppercase tracking-wider border px-1.5 py-0.5 md:block ${labelTone}`}
      >
        {entry.label}
      </span>
    </li>
  );
}

function roundPct(value: number) {
  const rounded = Math.round(value * 100) / 100;
  return rounded === 0 ? 0 : rounded;
}

function signedPct(value: number) {
  const safe = roundPct(value);
  return `${safe >= 0 ? "+" : ""}${safe.toFixed(2)}%`;
}

function formatAum(value: number) {
  const v = Number.isFinite(value) ? value : 0;
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(1)}k`;
  return `$${Math.round(v)}`;
}

function LeaderboardIdentity({ entry }: { entry: LeaderboardEntry }) {
  return (
    <Link
      href={`/diary/${entry.handle}`}
      className="flex min-h-9 min-w-0 flex-wrap items-center gap-2 text-text-hi transition-colors hover:text-accent-agent md:min-h-0"
    >
      <span className="min-w-0 truncate">
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
        <span className="text-[10px] text-accent-agent/70">
          {entry.distinctModels} models
        </span>
      )}
    </Link>
  );
}

function MobileMetric({
  label,
  value,
  className = "text-text-default",
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className={`mt-1 tabular-nums ${className}`}>{value}</p>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="border-brutal border-border-default bg-raised p-12 text-center space-y-4">
      <p className="text-sm font-mono text-text-lo">No live portfolios yet.</p>
      <p className="text-xs font-mono text-text-mut max-w-sm mx-auto">
        The leaderboard fills out as the agent has 24h of outcomes to compare
        against its own counterfactuals.
      </p>
      <div className="flex flex-col sm:flex-row items-center justify-center gap-3 pt-2">
        <Link
          href="/explore"
          className="inline-flex min-h-9 items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-4 py-2 font-mono text-xs text-accent-agent hover:border-accent-agent"
        >
          Explore demo portfolios
        </Link>
        <Link
          href="/about/regime"
          className="inline-flex min-h-9 items-center gap-2 border border-border-default bg-bg px-4 py-2 font-mono text-xs text-text-lo hover:text-text-hi"
        >
          How the leaderboard works
        </Link>
      </div>
    </div>
  );
}
