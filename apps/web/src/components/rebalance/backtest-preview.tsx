"use client";

import { useEffect, useState } from "react";
import { backtestApi, type BacktestResponse } from "@/lib/api";

interface Props {
  portfolioId: string | null;
  /** Optional explicit proposed weights. When omitted, the API falls back to
   *  the portfolio's stored `target_weight`. */
  proposed?: Array<{ symbol: string; targetWeight: number }>;
}

/**
 * Inline 30-day backtest preview rendered above the Approve button.
 *
 * Renders three states:
 *   1. Loading — slim shimmer line.
 *   2. Result — current vs proposed Δreturn + sharpe + drawdown.
 *   3. Error / unreliable — soft warning with what we know.
 */
export function BacktestPreview({ portfolioId, proposed }: Props) {
  const [data, setData] = useState<BacktestResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!portfolioId) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    backtestApi
      .preview(portfolioId, proposed)
      .then((res) => {
        if (!cancelled) setData(res);
      })
      .catch((e: unknown) => {
        if (!cancelled)
          setError(e instanceof Error ? e.message : "backtest failed");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [portfolioId, proposed]);

  if (loading) {
    return (
      <div className="bg-black/40 border border-white/5 p-3 mb-4 text-xs font-mono text-text-mut">
        <div className="flex items-center justify-between">
          <span>30d backtest</span>
          <span className="animate-pulse text-accent-agent">computing…</span>
        </div>
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="bg-black/40 border border-amber-500/20 p-3 mb-4 text-xs font-mono text-warn">
        Backtest unavailable
        {error && <span className="opacity-60"> · {error}</span>}
      </div>
    );
  }

  const delta = data.deltaTotalReturnPct;
  const deltaTone =
    delta > 0
      ? "text-accent-agent"
      : delta < 0
        ? "text-risk"
        : "text-text-default";
  const deltaSign = delta > 0 ? "+" : "";

  return (
    <div className="bg-black/40 border border-white/5 p-3 mb-4 text-xs font-mono space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-accent-agent/80">
          30d backtest{" "}
          <span className="text-text-mut">({data.windowDays}d window)</span>
        </span>
        {!data.reliable && (
          <span
            className="text-[10px] text-warn/80"
            title="Fewer than 5 daily snapshots — treat the numbers as directional only."
          >
            low data
          </span>
        )}
      </div>

      <div className="grid grid-cols-3 gap-2 text-[11px]">
        <Cell
          label="Δ return"
          value={`${deltaSign}${delta.toFixed(2)}%`}
          tone={deltaTone}
        />
        <Cell
          label="Sharpe (cur → prop)"
          value={`${data.current.sharpe.toFixed(2)} → ${data.proposed.sharpe.toFixed(2)}`}
        />
        <Cell
          label="Max DD (cur → prop)"
          value={`${data.current.maxDrawdownPct.toFixed(1)}% → ${data.proposed.maxDrawdownPct.toFixed(1)}%`}
        />
      </div>
    </div>
  );
}

function Cell({
  label,
  value,
  tone = "text-text-hi",
}: {
  label: string;
  value: string;
  tone?: string;
}) {
  return (
    <div className="bg-white/2 border border-white/5 p-2">
      <div className="text-[10px] text-text-mut">{label}</div>
      <div className={`mt-1 ${tone}`}>{value}</div>
    </div>
  );
}
