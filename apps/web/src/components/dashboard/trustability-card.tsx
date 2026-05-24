"use client";

import { useEffect, useState } from "react";
import { ShieldCheck } from "lucide-react";
import { trustabilityApi, type TrustabilityResponse } from "@/lib/api";
import { formatPercent, timeAgo } from "@/lib/utils";
import {
  BrutalCard as Card,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  BrutalCardBody as CardContent,
  ProvenanceLine,
} from "@aegis/ui";

type TrustLabel = NonNullable<TrustabilityResponse["label"]>;

const LABEL_TONE: Record<TrustLabel, string> = {
  excellent: "text-accent-agent border-accent-agent/40 bg-accent-agent/10",
  strong: "text-accent-agent/80 border-accent-agent/30 bg-accent-agent/5",
  stable: "text-text-hi border-border-default bg-bg",
  shaky: "text-warn border-warn/30 bg-warn/5",
  underperforming: "text-risk border-risk/30 bg-risk/5",
};

const EMPTY_PROGRESS: TrustabilityResponse["progress"] = {
  calibrationFloor: 50,
  agentDecisions7d: 0,
  eligibleOutcomes7d: 0,
  pendingRealRebalances7d: 0,
  completedRealRebalances7d: 0,
  distinctModels7d: 0,
  lastDecisionAt: null,
};

export function TrustabilityCard() {
  const [data, setData] = useState<TrustabilityResponse | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    trustabilityApi
      .me()
      .then((r) => {
        if (!cancelled) setData(r);
      })
      .catch(() => {
        // Quiet failure — the card just shows the starter copy.
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const progress = data?.progress ?? EMPTY_PROGRESS;
  const row = data?.row ?? null;

  return (
    <Shell>
      {loading ? (
        <LoadingState />
      ) : row ? (
        <ScoreState row={row} label={data?.label ?? null} progress={progress} />
      ) : (
        <EmptyState progress={progress} />
      )}
    </Shell>
  );
}

function LoadingState() {
  return (
    <TrustContent
      headline="Loading"
      title="Reading outcome history"
      body="Checking completed real plans, model coverage, and the calibration sample."
      progress={0}
      stats={[
        { label: "eligible outcomes", value: "..." },
        { label: "agent plans 7d", value: "..." },
        { label: "pending real", value: "..." },
        { label: "models used", value: "..." },
      ]}
    />
  );
}

function EmptyState({
  progress,
}: {
  progress: TrustabilityResponse["progress"];
}) {
  const floor = Math.max(progress.calibrationFloor, 1);
  const state = emptyStateCopy(progress);
  return (
    <TrustContent
      eyebrow="Outcome sample"
      headline={`${progress.eligibleOutcomes7d} / ${floor}`}
      title={state.title}
      body={state.body}
      progress={progress.eligibleOutcomes7d / floor}
      stats={progressStats(progress, floor)}
    />
  );
}

function ScoreState({
  row,
  label,
  progress,
}: {
  row: NonNullable<TrustabilityResponse["row"]>;
  label: TrustabilityResponse["label"];
  progress: TrustabilityResponse["progress"];
}) {
  const floor = Math.max(progress.calibrationFloor, 1);
  const isPreCalibration = row.decisionsExecuted < floor;
  const tone = label ? LABEL_TONE[label] : LABEL_TONE.stable;
  const badge =
    !isPreCalibration && label ? (
      <LabelBadge label={label} tone={tone} />
    ) : undefined;

  return (
    <TrustContent
      eyebrow={isPreCalibration ? "Calibration sample" : "7-day edge"}
      headline={scoreHeadline(row, floor, isPreCalibration)}
      title={
        isPreCalibration
          ? "Sample building"
          : `${label ?? "stable"} trust score`
      }
      body={scoreBody(floor, isPreCalibration)}
      progress={isPreCalibration ? row.decisionsExecuted / floor : 1}
      badge={badge}
      stats={scoreStats(row, floor, isPreCalibration)}
    />
  );
}

function scoreHeadline(
  row: NonNullable<TrustabilityResponse["row"]>,
  floor: number,
  isPreCalibration: boolean,
): string {
  if (isPreCalibration) return `${row.decisionsExecuted} / ${floor}`;
  const sign = row.trustabilityDelta > 0 ? "+" : "";
  return `${sign}${row.trustabilityDelta.toFixed(2)}%`;
}

function scoreBody(floor: number, isPreCalibration: boolean): string {
  if (isPreCalibration) {
    return `Score unlocks after ${floor} completed real outcomes. The card is tracking the sample before showing a calibrated delta.`;
  }
  return "Compares completed real plans against their counterfactual outcome over the trailing 7-day window.";
}

function LabelBadge({ label, tone }: { label: string; tone: string }) {
  return (
    <span
      className={`border px-2 py-1 font-mono text-[10px] uppercase tracking-wider ${tone}`}
    >
      {label}
    </span>
  );
}

function scoreStats(
  row: NonNullable<TrustabilityResponse["row"]>,
  floor: number,
  isPreCalibration: boolean,
): Array<{ label: string; value: string; tone?: "positive" | "negative" }> {
  return [
    {
      label: isPreCalibration ? "eligible outcomes" : "decisions",
      value: isPreCalibration
        ? `${row.decisionsExecuted} / ${floor}`
        : String(row.decisionsExecuted),
    },
    {
      label: "models routed",
      value: String(row.distinctModels),
    },
    {
      label: "avg 7d return",
      value: formatPercent(row.avg7dReturn),
      tone: row.avg7dReturn >= 0 ? "positive" : "negative",
    },
    {
      label: "last plan",
      value: row.lastDecisionAt ? timeAgo(row.lastDecisionAt) : "none",
    },
  ];
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <Card className="overflow-hidden">
      <CardHeader className="min-h-[56px] shrink-0">
        <CardTitle className="flex min-w-0 items-center gap-2">
          <ShieldCheck className="h-3.5 w-3.5 shrink-0 text-accent-agent" />
          <span className="truncate">Trust Score</span>
        </CardTitle>
        <span className="hidden font-mono text-[10px] text-text-mut md:block">
          Outcome-backed agent reliability
        </span>
      </CardHeader>
      <CardContent className="p-0 font-mono">
        {children}
        <div className="border-t border-border-default px-4 py-3">
          <ProvenanceLine
            source="completed plan outcomes"
            freshness="7d window"
          />
        </div>
      </CardContent>
    </Card>
  );
}

function TrustContent({
  eyebrow = "Trust score",
  headline,
  title,
  body,
  progress,
  stats,
  badge,
}: {
  eyebrow?: string;
  headline: string;
  title: string;
  body: string;
  progress: number;
  stats: Array<{
    label: string;
    value: string;
    tone?: "positive" | "negative";
  }>;
  badge?: React.ReactNode;
}) {
  return (
    <div className="grid xl:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
      <section className="min-w-0 border-b border-border-default p-3 sm:p-4 xl:border-b-0 xl:border-r">
        <div className="flex min-w-0 items-center justify-between gap-3">
          <span className="font-mono text-[11px] uppercase tracking-wider text-accent-agent/70">
            {eyebrow}
          </span>
          {badge}
        </div>
        <div className="mt-3 grid grid-cols-[auto_minmax(0,1fr)] items-start gap-x-4 gap-y-2">
          <span className="min-w-0 font-mono text-[1.85rem] font-semibold leading-none text-text-hi tabular-nums sm:text-4xl">
            {headline}
          </span>
          <div className="min-w-0">
            <span className="block min-w-0 text-sm font-semibold leading-snug text-text-hi">
              {title}
            </span>
            <p className="mt-1 max-w-3xl text-xs leading-relaxed text-text-lo">
              {body}
            </p>
          </div>
          <div className="col-span-2">
            <ProgressRail value={progress} />
          </div>
        </div>
      </section>

      <section className="grid grid-cols-2 sm:grid-cols-4 xl:grid-cols-2">
        {stats.map((stat) => (
          <Stat key={stat.label} {...stat} />
        ))}
      </section>
    </div>
  );
}

function ProgressRail({ value }: { value: number }) {
  const pct = Math.max(0, Math.min(value, 1)) * 100;
  return (
    <div className="h-1.5 border border-border-default bg-bg">
      <div className="h-full bg-accent-agent" style={{ width: `${pct}%` }} />
    </div>
  );
}

function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "positive" | "negative";
}) {
  const valueClass =
    tone === "positive"
      ? "text-accent-pnl"
      : tone === "negative"
        ? "text-risk"
        : "text-text-default";

  return (
    <div className="min-h-[58px] min-w-0 border-b border-r border-border-default bg-bg px-3 py-2.5 text-[11px] last:border-r-0 even:border-r-0 [&:nth-child(3)]:border-b-0 [&:nth-child(4)]:border-b-0 sm:border-b-0 sm:even:border-r sm:[&:nth-child(4n)]:border-r-0 xl:border-b xl:[&:nth-child(2n)]:border-r-0 xl:[&:nth-child(3)]:border-b-0 xl:[&:nth-child(4)]:border-b-0">
      <div className="truncate text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </div>
      <div
        className={`mt-1 truncate font-semibold tabular-nums ${valueClass}`}
        title={value}
      >
        {value}
      </div>
    </div>
  );
}

function emptyStateCopy(progress: TrustabilityResponse["progress"]): {
  title: string;
  body: string;
} {
  if (progress.agentDecisions7d === 0) {
    return {
      title: "Waiting for first decision",
      body: "Ask the agent for a plan. Trust Score starts after a real rebalance completes and its outcome enters the 7-day window.",
    };
  }

  if (progress.pendingRealRebalances7d > 0) {
    return {
      title: "Execution outcome pending",
      body: "A real plan exists, but Trust Score only counts it after completion and outcome capture.",
    };
  }

  return {
    title: "Plans drafted, no eligible outcomes",
    body: "The account has agent plans, but the score only uses completed real rebalances with comparable 7-day outcomes.",
  };
}

function progressStats(
  progress: TrustabilityResponse["progress"],
  floor: number,
): Array<{ label: string; value: string }> {
  return [
    {
      label: "eligible outcomes",
      value: `${progress.eligibleOutcomes7d} / ${floor}`,
    },
    {
      label: "agent plans 7d",
      value: String(progress.agentDecisions7d),
    },
    {
      label: "pending real",
      value: String(progress.pendingRealRebalances7d),
    },
    {
      label: "models used",
      value: String(progress.distinctModels7d),
    },
  ];
}
