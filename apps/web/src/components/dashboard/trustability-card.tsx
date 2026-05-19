"use client";

import { useEffect, useState } from "react";
import { ShieldCheck } from "lucide-react";
import { trustabilityApi, type TrustabilityResponse } from "@/lib/api";
import { ProvenanceLine } from "@aegis/ui";

const LABEL_TONE: Record<string, string> = {
  excellent: "text-accent-pnl border-emerald-500/30 bg-emerald-500/5",
  strong: "text-accent-pnl/70 border-emerald-500/20 bg-emerald-500/3",
  stable: "text-text-hi border-white/15 bg-white/3",
  shaky: "text-warn border-amber-500/30 bg-amber-500/5",
  underperforming: "text-risk border-rose-500/30 bg-rose-500/5",
};

/**
 * Dashboard hero card showing the user's trustability score — the agent's
 * 7d realized return delta vs its own counterfactual. Built from the
 * `v_trustability_per_user` view (migration 0005); zero-decision users see
 * a starter copy instead of a number.
 */
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

  if (loading) {
    return <Shell>Loading agent trust score…</Shell>;
  }

  if (!data || !data.row) {
    return (
      <Shell>
        <p className="text-sm text-text-default mb-1">No score yet</p>
        <p className="text-xs text-text-lo">
          Run a few rebalances; the score becomes available after the agent has
          24h of outcomes to compare against its own counterfactuals.
        </p>
      </Shell>
    );
  }

  const { row, label } = data;
  const sign = row.trustabilityDelta > 0 ? "+" : "";
  const tone = label ? LABEL_TONE[label] : LABEL_TONE.stable;
  // Calibration floor — the histogram-bin calibrator needs ≥50 outcomes
  // before the trust score is meaningful. Showing "0.00%" with 5 decisions
  // looks like the agent is broken; show the sample-size progress instead.
  const CALIBRATION_FLOOR = 50;
  const isPreCalibration = row.decisionsExecuted < CALIBRATION_FLOOR;

  // Horizontal layout — the card used to be full-width vertical with one
  // tiny "models routed" stat dangling at the bottom; now headline lives on
  // the left, explainer in the middle, and stat tile on the right.
  return (
    <Shell>
      <div className="grid grid-cols-1 md:grid-cols-[auto_1fr_auto] md:items-center gap-6">
        <div>
          <div className="flex items-baseline justify-between gap-3 mb-2">
            <span className="text-[11px] uppercase tracking-wider text-accent-agent/70 font-mono">
              Agent trust score
            </span>
            {!isPreCalibration && (
              <span
                className={`text-[10px] font-mono uppercase tracking-wider border px-1.5 py-0.5 ${tone}`}
              >
                {label}
              </span>
            )}
          </div>
          {isPreCalibration ? (
            <div className="flex items-baseline gap-2">
              <span className="text-3xl font-mono text-text-mut tabular-nums">
                {row.decisionsExecuted}
                <span className="text-text-lo text-lg">
                  {" / "}
                  {CALIBRATION_FLOOR}
                </span>
              </span>
              <span className="text-[11px] text-text-lo">decisions</span>
            </div>
          ) : (
            <div className="flex items-baseline gap-2">
              <span className="text-3xl font-mono text-text-hi tabular-nums">
                {sign}
                {row.trustabilityDelta.toFixed(2)}%
              </span>
              <span className="text-[11px] text-text-lo">
                vs counterfactual · 7d
              </span>
            </div>
          )}
        </div>

        <p className="text-[11px] text-text-mut font-mono leading-relaxed">
          {isPreCalibration
            ? `Trust score unlocks at ${CALIBRATION_FLOOR} executed decisions — the calibrator needs that sample to compare agent outcomes against the counterfactual.`
            : "Realized vs counterfactual outcome delta over the trailing 7-day window."}
        </p>

        <div
          className={`grid gap-2 text-[11px] font-mono ${
            isPreCalibration ? "grid-cols-1" : "grid-cols-3"
          }`}
        >
          {!isPreCalibration && (
            <Stat label="decisions" value={String(row.decisionsExecuted)} />
          )}
          <Stat label="models routed" value={String(row.distinctModels)} />
          {!isPreCalibration && (
            <Stat
              label="avg 7d return"
              value={`${row.avg7dReturn >= 0 ? "+" : ""}${row.avg7dReturn.toFixed(2)}%`}
            />
          )}
        </div>
      </div>
    </Shell>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <div className="border-2 border-white/10 bg-[#141414] p-4 space-y-1">
      <div className="flex items-center gap-2 mb-1">
        <ShieldCheck className="w-3.5 h-3.5 text-accent-agent" />
        <span className="text-xs font-semibold text-text-hi">Trustability</span>
      </div>
      {children}
      <div className="pt-2 border-t border-white/10">
        <ProvenanceLine
          source="on-chain outcomes + counterfactuals"
          freshness="7d window"
        />
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-white/2 border border-white/5 px-2 py-1.5">
      <div className="text-[10px] text-text-lo">{label}</div>
      <div className="text-text-default tabular-nums">{value}</div>
    </div>
  );
}
