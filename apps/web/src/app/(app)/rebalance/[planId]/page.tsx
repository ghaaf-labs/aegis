"use client";

import { useEffect, useState, use } from "react";
import { useRouter } from "next/navigation";

import { rebalanceApi } from "@/lib/api";
import { ApprovalModal } from "@/components/rebalance/approval-modal";
import { ExecutionTrace } from "@/components/rebalance/execution-trace";

const SSE_URL = process.env.NEXT_PUBLIC_SSE_URL ?? "http://localhost:8080/sse";

interface PageProps {
  params: Promise<{ planId: string }>;
}

/**
 * Rebalance review + execution page. The portfolio dashboard navigates here
 * after `rebalanceApi.plan()` succeeds; this page renders the approval modal
 * first, then transitions to the realtime execution trace once the user
 * approves.
 */
export default function RebalancePage({ params }: PageProps) {
  const { planId } = use(params);
  const router = useRouter();

  const [showApproval, setShowApproval] = useState(true);
  const [estimatedFee, setEstimatedFee] = useState(0);
  const [plan, setPlan] = useState<{
    rebalanceId: string;
    decisionId: string;
    totalLegs: number;
    legs: Array<{
      legIndex: number;
      kind: string;
      srcChain: string | null;
      destChain: string | null;
      srcSymbol: string | null;
      destSymbol: string | null;
      amountUsdc: number;
    }>;
  } | null>(null);
  const [approved, setApproved] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void rebalanceApi
      .get(planId)
      .then((detail) => {
        if (cancelled) return;
        // Pre-approval: render the approval modal.
        setPlan({
          rebalanceId: detail.id,
          decisionId: detail.decisionId,
          totalLegs: detail.totalLegs,
          legs: detail.legs.map((l) => ({
            legIndex: l.legIndex,
            kind: l.kind,
            srcChain: l.srcChain,
            destChain: l.destChain,
            srcSymbol: l.srcSymbol,
            destSymbol: l.destSymbol,
            amountUsdc: l.amountUsdc,
          })),
        });
        setEstimatedFee(detail.totalGasUsdc ?? 0);
        // If the plan is already past 'planned' state, skip approval modal.
        if (detail.status !== "planned") {
          setShowApproval(false);
          setApproved(true);
        }
      })
      .catch((e) =>
        setLoadError(e instanceof Error ? e.message : "load failed"),
      );
    return () => {
      cancelled = true;
    };
  }, [planId]);

  if (loadError) {
    return (
      <main className="p-8 text-rose-300 font-mono text-sm">
        Failed to load rebalance plan: {loadError}
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-[#0A0A0A] text-white px-6 py-8">
      <div className="max-w-3xl mx-auto space-y-6">
        <header>
          <p className="font-mono text-[11px] tracking-wider text-cyan-400 uppercase">
            Rebalance · {planId.slice(0, 8)}…
          </p>
          <h1 className="text-2xl font-bold mt-1">
            {approved ? "Execution in progress" : "Review the plan"}
          </h1>
        </header>

        {approved ? (
          <ExecutionTrace rebalanceId={planId} sseUrl={SSE_URL} />
        ) : (
          <p className="text-sm text-gray-400">
            The agent has built a cross-chain plan. Review the legs, then
            approve to settle on Arc + Base.
          </p>
        )}

        <ApprovalModal
          open={showApproval && plan !== null}
          plan={plan}
          estimatedFeeUsdc={estimatedFee}
          onApproved={() => {
            setShowApproval(false);
            setApproved(true);
          }}
          onClose={() => {
            setShowApproval(false);
            router.back();
          }}
        />
      </div>
    </main>
  );
}
