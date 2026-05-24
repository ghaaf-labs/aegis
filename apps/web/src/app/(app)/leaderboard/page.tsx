import type { Metadata } from "next";
import type { ReactNode } from "react";
import Link from "next/link";
import {
  Activity,
  ArrowRight,
  BookOpen,
  Clock3,
  ShieldCheck,
  Trophy,
} from "lucide-react";
import { ModelBadge } from "@aegis/ui";
import { pageMetadata } from "@/lib/seo";
import { cn } from "@/lib/utils";

export const metadata: Metadata = pageMetadata({
  title: "Leaderboard — Aegis",
  description:
    "Public Aegis portfolios ranked by realized 7d return vs counterfactual.",
  path: "/leaderboard",
});

export const dynamic = "force-dynamic";
export const revalidate = 0;

interface LeaderboardEntry {
  userId: string;
  handle: string;
  decisionsExecuted: number;
  eligibleOutcomes: number;
  decisionsPerWeek: number;
  distinctModels: number;
  avg7dReturn: number;
  trustabilityDelta: number;
  lastDecisionAt: string | null;
  aumUsd: number;
  label: "excellent" | "strong" | "stable" | "shaky" | "underperforming";
  recentModelSlug?: string;
  recentCriticVerdict?: {
    verdict?: "approved" | "revised" | "abstained" | "veto";
    demandsRevision?: boolean;
  };
}

const LABEL_TONE: Record<LeaderboardEntry["label"], string> = {
  excellent: "border-accent-pnl/40 bg-accent-pnl/10 text-accent-pnl",
  strong: "border-accent-pnl/30 bg-accent-pnl/5 text-accent-pnl",
  stable: "border-border-default bg-raised text-text-hi",
  shaky: "border-warn/40 bg-warn/10 text-warn",
  underperforming: "border-risk/40 bg-risk/10 text-risk",
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
  const eligibleOutcomes = rows.reduce(
    (sum, row) => sum + row.eligibleOutcomes,
    0,
  );
  const totalAumUsd = rows.reduce(
    (sum, row) => sum + safeNumber(row.aumUsd),
    0,
  );
  const weightedDelta =
    eligibleOutcomes > 0
      ? rows.reduce(
          (sum, row) => sum + row.trustabilityDelta * row.eligibleOutcomes,
          0,
        ) / eligibleOutcomes
      : 0;
  const lastDecisionAt = rows
    .map((row) => row.lastDecisionAt)
    .filter((value): value is string => Boolean(value))
    .sort((a, b) => new Date(b).getTime() - new Date(a).getTime())[0];

  return (
    <div className="mx-auto w-full max-w-[1400px] space-y-5 md:space-y-6">
      <section className="border-brutal border-border-default bg-surface">
        <div className="grid gap-4 border-b border-border-default px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-start md:px-5">
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              <Trophy className="h-4 w-4 shrink-0 text-accent-pnl" />
              <h1 className="truncate font-mono text-2xl font-semibold text-text-hi">
                Leaderboard
              </h1>
            </div>
            <p className="mt-2 max-w-3xl text-sm leading-relaxed text-text-lo">
              Public diaries ranked by realized 7d return against the strategist
              counterfactual. Private diaries are excluded from both ranking and
              lookup.
            </p>
          </div>
          <Link
            href="/settings"
            className="inline-flex min-h-10 items-center justify-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 font-mono text-xs font-semibold text-accent-agent hover:border-accent-agent"
          >
            Privacy settings
            <ArrowRight className="h-3.5 w-3.5" />
          </Link>
        </div>

        <div className="grid grid-cols-2 border-b border-border-default lg:grid-cols-4">
          <SummaryCell
            icon={<ShieldCheck className="h-4 w-4" />}
            label="Public diaries"
            value={String(rows.length)}
            detail="opt-in only"
          />
          <SummaryCell
            icon={<Activity className="h-4 w-4" />}
            label="Eligible outcomes"
            value={String(eligibleOutcomes)}
            detail="24h windows"
            tone="agent"
          />
          <SummaryCell
            icon={<Trophy className="h-4 w-4" />}
            label="Avg delta"
            value={signedPct(weightedDelta)}
            detail="vs counterfactual"
            tone={weightedDelta >= 0 ? "pnl" : "risk"}
          />
          <SummaryCell
            icon={<Clock3 className="h-4 w-4" />}
            label="Last decision"
            value={lastDecisionAt ? relativeTime(lastDecisionAt) : "none"}
            detail={formatAum(totalAumUsd)}
          />
        </div>

        {rows.length === 0 ? (
          <EmptyState />
        ) : (
          <>
            <TableHeader />
            <ol>
              {rows.map((row, index) => (
                <Row key={row.userId} entry={row} rank={index + 1} />
              ))}
            </ol>
            <div className="border-t border-border-default px-4 py-3 font-mono text-[10px] text-text-mut md:px-5">
              via completed real executions + public diary opt-in
            </div>
          </>
        )}
      </section>
    </div>
  );
}

const GRID_COLS =
  "lg:grid-cols-[44px_minmax(220px,1.5fr)_110px_100px_92px_92px_108px_94px]";

function TableHeader() {
  return (
    <div
      className={cn(
        "hidden border-b border-border-default bg-bg/50 px-4 py-3 font-mono text-[10px] uppercase tracking-widest text-text-mut md:px-5 lg:grid lg:gap-3",
        GRID_COLS,
      )}
    >
      <span>#</span>
      <span>Public diary</span>
      <span className="text-right">Delta</span>
      <span className="text-right">7d return</span>
      <span className="text-right">AUM</span>
      <span className="text-right">Outcomes</span>
      <span className="text-right">Latest</span>
      <span className="text-right">Open</span>
    </div>
  );
}

function Row({ entry, rank }: { entry: LeaderboardEntry; rank: number }) {
  const hasOutcome = entry.eligibleOutcomes > 0;
  const deltaTone = toneForSignedValue(
    hasOutcome ? entry.trustabilityDelta : 0,
    !hasOutcome,
  );
  const returnTone = toneForSignedValue(
    hasOutcome ? entry.avg7dReturn : 0,
    !hasOutcome,
  );
  const label = hasOutcome ? entry.label : "collecting";
  const labelTone = hasOutcome
    ? LABEL_TONE[entry.label]
    : "border-warn/35 bg-warn/5 text-warn";

  return (
    <li
      className={cn(
        "border-b border-border-default px-4 py-4 font-mono text-xs last:border-0 hover:bg-white/2 md:px-5 lg:grid lg:items-center lg:gap-3 lg:py-3",
        GRID_COLS,
      )}
    >
      <div className="space-y-3 lg:hidden">
        <div className="flex min-w-0 items-start gap-3">
          <span className="mt-0.5 shrink-0 text-text-mut">
            {rank.toString().padStart(2, "0")}
          </span>
          <LeaderboardIdentity entry={entry} />
        </div>
        <div className="grid grid-cols-2 gap-2">
          <MobileMetric
            label="Delta"
            value={hasOutcome ? signedPct(entry.trustabilityDelta) : "pending"}
            className={deltaTone}
          />
          <MobileMetric
            label="7d return"
            value={hasOutcome ? signedPct(entry.avg7dReturn) : "pending"}
            className={returnTone}
          />
          <MobileMetric
            label="AUM"
            value={formatAum(entry.aumUsd)}
            className="text-accent-pnl"
          />
          <MobileMetric
            label="Outcomes"
            value={`${entry.eligibleOutcomes}/${entry.decisionsExecuted}`}
            className="text-accent-agent"
          />
        </div>
      </div>

      <span className="hidden text-text-mut lg:block">
        {rank.toString().padStart(2, "0")}
      </span>
      <div className="hidden min-w-0 lg:block">
        <LeaderboardIdentity entry={entry} />
      </div>
      <span
        className={cn("hidden text-right tabular-nums lg:block", deltaTone)}
      >
        {hasOutcome ? signedPct(entry.trustabilityDelta) : "pending"}
      </span>
      <span
        className={cn("hidden text-right tabular-nums lg:block", returnTone)}
      >
        {hasOutcome ? signedPct(entry.avg7dReturn) : "pending"}
      </span>
      <span className="hidden text-right tabular-nums text-accent-pnl lg:block">
        {formatAum(entry.aumUsd)}
      </span>
      <span className="hidden text-right tabular-nums text-accent-agent lg:block">
        {entry.eligibleOutcomes}/{entry.decisionsExecuted}
      </span>
      <span className="hidden text-right tabular-nums text-text-lo lg:block">
        {entry.lastDecisionAt ? relativeTime(entry.lastDecisionAt) : "none"}
      </span>
      <span
        className={cn(
          "hidden justify-self-end border px-1.5 py-0.5 text-right text-[10px] uppercase tracking-wider lg:block",
          labelTone,
        )}
      >
        {label}
      </span>
    </li>
  );
}

function SummaryCell({
  detail,
  icon,
  label,
  tone = "default",
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  tone?: "default" | "pnl" | "agent" | "risk";
  value: string;
}) {
  return (
    <div className="min-h-[82px] border-r border-border-default px-4 py-3 last:border-r-0 even:border-r-0 lg:even:border-r lg:[&:nth-child(4n)]:border-r-0">
      <div className="flex items-center gap-2 text-text-mut">
        <span className={cn("shrink-0", toneClass(tone))}>{icon}</span>
        <p className="truncate text-[10px] uppercase tracking-widest">
          {label}
        </p>
      </div>
      <p
        className={cn(
          "mt-2 text-xl font-semibold tabular-nums",
          toneClass(tone),
        )}
      >
        {value}
      </p>
      <p className="mt-1 truncate text-[10px] text-text-lo">{detail}</p>
    </div>
  );
}

function LeaderboardIdentity({ entry }: { entry: LeaderboardEntry }) {
  return (
    <Link
      href={`/diary/${entry.handle}`}
      className="group flex min-w-0 flex-wrap items-center gap-2 text-text-hi hover:text-accent-agent"
    >
      <BookOpen className="h-3.5 w-3.5 shrink-0 text-accent-agent/70" />
      <span className="min-w-0 truncate text-sm font-semibold">
        <span className="text-text-mut">0x</span>
        {entry.handle}
      </span>
      {entry.recentModelSlug && <ModelBadge model={entry.recentModelSlug} />}
      {entry.recentCriticVerdict && (
        <span
          className={cn(
            "border px-1 py-0.5 text-[9px] uppercase tracking-wider",
            criticTone(entry.recentCriticVerdict),
          )}
        >
          {criticLabel(entry.recentCriticVerdict)}
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
  className = "text-text-hi",
  label,
  value,
}: {
  className?: string;
  label: string;
  value: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className={cn("mt-1 tabular-nums", className)}>{value}</p>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="px-5 py-12 text-center">
      <div className="mx-auto flex h-10 w-10 items-center justify-center border border-accent-agent/40 bg-accent-agent/10 text-accent-agent">
        <Trophy className="h-5 w-5" />
      </div>
      <p className="mt-4 font-mono text-sm font-semibold text-text-hi">
        No ranked public diaries yet
      </p>
      <p className="mx-auto mt-2 max-w-md text-sm leading-relaxed text-text-lo">
        A portfolio appears here after its owner enables the public diary and a
        real execution has enough outcome data to compare against its
        counterfactual.
      </p>
      <div className="mt-5 flex flex-col items-center justify-center gap-3 sm:flex-row">
        <Link
          href="/explore"
          className="inline-flex min-h-10 items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-4 font-mono text-xs font-semibold text-accent-agent hover:border-accent-agent"
        >
          Explore portfolios
        </Link>
        <Link
          href="/settings"
          className="inline-flex min-h-10 items-center gap-2 border border-border-default bg-bg px-4 font-mono text-xs font-semibold text-text-lo hover:text-text-hi"
        >
          Diary privacy
        </Link>
      </div>
    </div>
  );
}

function toneForSignedValue(value: number, muted = false) {
  if (muted) return "text-text-mut";
  if (value > 0) return "text-accent-pnl";
  if (value < 0) return "text-risk";
  return "text-text-hi";
}

function toneClass(tone: "default" | "pnl" | "agent" | "risk") {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "risk") return "text-risk";
  return "text-text-hi";
}

function criticLabel(
  verdict: NonNullable<LeaderboardEntry["recentCriticVerdict"]>,
) {
  if (verdict.verdict) return verdict.verdict;
  return verdict.demandsRevision ? "revised" : "approved";
}

function criticTone(
  verdict: NonNullable<LeaderboardEntry["recentCriticVerdict"]>,
) {
  const label = criticLabel(verdict);
  if (label === "revised" || label === "veto") {
    return "border-risk/40 text-risk";
  }
  return "border-accent-agent/40 text-accent-agent";
}

function roundPct(value: number) {
  const rounded = Math.round(safeNumber(value) * 100) / 100;
  return rounded === 0 ? 0 : rounded;
}

function signedPct(value: number) {
  const safe = roundPct(value);
  return `${safe >= 0 ? "+" : ""}${safe.toFixed(2)}%`;
}

function formatAum(value: number) {
  const safe = safeNumber(value);
  if (safe >= 1_000_000) return `$${(safe / 1_000_000).toFixed(1)}M`;
  if (safe >= 1_000) return `$${(safe / 1_000).toFixed(1)}K`;
  return `$${safe.toFixed(safe >= 100 ? 0 : 2)}`;
}

function relativeTime(value: string) {
  const seconds = Math.max(
    0,
    Math.floor((Date.now() - new Date(value).getTime()) / 1000),
  );
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

function safeNumber(value: number) {
  return Number.isFinite(value) ? value : 0;
}
