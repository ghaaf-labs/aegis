"use client";

import { useEffect, useState } from "react";

import { useEventSource } from "@/lib/sse";
import { rebalanceApi, analyticsApi } from "@/lib/api";
import { buildShareIntent } from "@/lib/share";
import type { LegStatus } from "@/types";

import { LegCard } from "./leg-card";

export interface ExecutionTraceProps {
  rebalanceId: string;
  /** Auth-aware SSE URL (with token query, etc.). */
  sseUrl: string;
}

interface InternalLeg {
  id: string;
  legIndex: number;
  kind: string;
  srcChain: string | null;
  destChain: string | null;
  srcSymbol: string | null;
  destSymbol: string | null;
  amountUsdc: number;
  status: LegStatus;
  txHash: string | null;
  failureReason: string | null;
}

/**
 * Realtime trace of a single rebalance plan: fetches the initial leg list
 * via REST and applies incremental SSE updates so each transition lands
 * in the UI within ~50ms of the executor emitting it.
 */
export function ExecutionTrace({ rebalanceId, sseUrl }: ExecutionTraceProps) {
  const [legs, setLegs] = useState<InternalLeg[]>([]);
  const [status, setStatus] = useState<string>("loading…");
  const [completed, setCompleted] = useState<number>(0);
  const [total, setTotal] = useState<number>(0);
  const [decisionId, setDecisionId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void rebalanceApi.get(rebalanceId).then((plan) => {
      if (cancelled) return;
      setStatus(plan.status);
      setTotal(plan.totalLegs);
      setCompleted(plan.completedLegs);
      setDecisionId(plan.decisionId);
      setLegs(
        plan.legs.map((l) => ({
          id: l.id,
          legIndex: l.legIndex,
          kind: l.kind,
          srcChain: l.srcChain,
          destChain: l.destChain,
          srcSymbol: l.srcSymbol,
          destSymbol: l.destSymbol,
          amountUsdc: l.amountUsdc,
          status: l.status as LegStatus,
          txHash: l.txHash,
          failureReason: l.failureReason,
        })),
      );
    });
    return () => {
      cancelled = true;
    };
  }, [rebalanceId]);

  useEventSource(sseUrl, {
    "rebalance.leg.update": (data) => {
      // Drop events from other plans. With H3 fixed, every leg update
      // carries its parent rebalanceId so the same component can be
      // mounted twice (different plans) without crosstalk.
      if (data.rebalanceId !== rebalanceId) return;
      setLegs((prev) =>
        prev.map((leg) =>
          leg.legIndex === data.legIndex
            ? {
                ...leg,
                status: data.status as LegStatus,
                txHash: (data.txHash ?? leg.txHash) as string | null,
                failureReason: data.failureReason ?? leg.failureReason,
              }
            : leg,
        ),
      );
      if (data.status === "confirmed") {
        setCompleted((c) => c + 1);
      }
      if (data.status === "failed") {
        setStatus("failed");
      }
    },
    "rebalance.plan.created": (data) => {
      if (data.id === rebalanceId) {
        setStatus(data.status);
      }
    },
  } as Parameters<typeof useEventSource>[1]);

  const progressPct = total > 0 ? Math.round((completed / total) * 100) : 0;

  return (
    <section className="space-y-3">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">Execution trace</h2>
        <div className="flex items-center gap-3 font-mono text-xs">
          <span className="text-gray-400">
            {completed} / {total} legs
          </span>
          <span
            className={
              status === "completed"
                ? "text-emerald-300"
                : status === "failed"
                  ? "text-rose-300"
                  : "text-cyan-300"
            }
          >
            {status.toUpperCase()}
          </span>
        </div>
      </header>
      <div className="h-1.5 bg-white/5 overflow-hidden">
        <div
          className="h-full bg-emerald-400 transition-all duration-500"
          style={{ width: `${progressPct}%` }}
        />
      </div>
      <div className="space-y-2">
        {legs.length === 0 ? (
          <p className="text-sm text-gray-500">No legs yet.</p>
        ) : (
          legs
            .slice()
            .sort((a, b) => a.legIndex - b.legIndex)
            .map((leg) => (
              <LegCard
                key={leg.id}
                legIndex={leg.legIndex}
                kind={leg.kind}
                srcChain={leg.srcChain}
                destChain={leg.destChain}
                srcSymbol={leg.srcSymbol}
                destSymbol={leg.destSymbol}
                amountUsdc={leg.amountUsdc}
                status={leg.status}
                txHash={leg.txHash}
                failureReason={leg.failureReason}
              />
            ))
        )}
      </div>
      {status === "completed" && decisionId && (
        <ShareBlock decisionId={decisionId} />
      )}
    </section>
  );
}

function ShareBlock({ decisionId }: { decisionId: string }) {
  const handle = () => {
    const { intentUrl } = buildShareIntent({
      decisionId,
      summary: "rebalance executed",
      realizedPct: null,
    });
    void analyticsApi.track("share.opened", { decisionId });
    window.open(intentUrl, "_blank", "noopener,noreferrer");
  };
  return (
    <div className="mt-4 border-2 border-emerald-500/30 bg-emerald-500/5 p-3 flex items-center justify-between">
      <div>
        <p className="text-xs font-semibold text-emerald-200">
          Rebalance complete
        </p>
        <p className="text-[11px] text-emerald-200/70 mt-0.5">
          Share what the agent did — the OG card pulls the regime + outcome
          automatically.
        </p>
      </div>
      <button
        type="button"
        onClick={handle}
        className="px-3 py-1.5 text-xs font-semibold border-2 border-emerald-300 bg-emerald-500 text-black hover:bg-emerald-400 transition-colors"
      >
        Share to X
      </button>
    </div>
  );
}
