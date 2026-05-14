"use client";

import { useState } from "react";

import { rebalanceApi, type RebalancePlanResponse } from "@/lib/api";
import { cn } from "@/lib/utils";

function formatRelativeSeconds(at: Date): string {
  const secs = Math.max(0, Math.round((Date.now() - at.getTime()) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.round(secs / 60)}m ago`;
  return `${Math.round(secs / 3600)}h ago`;
}

export interface ApprovalModalProps {
  open: boolean;
  plan: RebalancePlanResponse | null;
  estimatedFeeUsdc: number;
  /** When the fee number was fetched. Drives the provenance line. */
  feeFetchedAt?: Date | null;
  /** Where the fee came from — `plan` is the planner-time stored value;
   *  `paymaster` is a live quote from `GET /paymaster/estimate`. */
  feeSource?: "plan" | "paymaster";
  /** Optional per-user / per-portfolio context surfaced in the header. */
  portfolioName?: string;
  onApproved: (rebalanceId: string) => void;
  onClose: () => void;
}

const KIND_LABEL: Record<string, string> = {
  local_swap: "Swap",
  cross_chain_burn: "CCTP burn",
  cross_chain_mint: "CCTP mint + hook",
  park_usyc: "Park → USYC",
  redeem_usyc: "Redeem ← USYC",
  fx_stablefx: "StableFX",
};

export function ApprovalModal({
  open,
  plan,
  estimatedFeeUsdc,
  feeFetchedAt,
  feeSource = "plan",
  portfolioName,
  onApproved,
  onClose,
}: ApprovalModalProps) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open || !plan) return null;

  const handleApprove = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await rebalanceApi.execute(plan.rebalanceId);
      onApproved(plan.rebalanceId);
    } catch (e) {
      setError(e instanceof Error ? e.message : "approval failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
      <div className="w-full max-w-xl bg-[#141414] border-2 border-white/15 shadow-[8px_8px_0_0_#000]">
        <header className="px-6 py-4 border-b border-white/10 flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold text-white">
              Approve rebalance
            </h2>
            {portfolioName && (
              <p className="text-[11px] font-mono text-gray-400 mt-1">
                {portfolioName}
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-gray-400 hover:text-white"
            aria-label="Close"
          >
            ×
          </button>
        </header>

        <div className="px-6 py-4">
          <p className="text-sm text-gray-300 mb-3">
            The agent has planned <strong>{plan.totalLegs}</strong> leg
            {plan.totalLegs === 1 ? "" : "s"} to bring your portfolio to its
            target. One click executes everything; SSE will stream per-leg
            updates as they confirm.
          </p>

          <ol className="space-y-2 mb-4">
            {plan.legs.map((leg) => (
              <li
                key={leg.legIndex}
                className="flex justify-between text-xs font-mono border border-white/5 p-2"
              >
                <span>
                  <span className="text-gray-500 mr-2">
                    {String(leg.legIndex + 1).padStart(2, "0")}
                  </span>
                  <span className="text-white">
                    {KIND_LABEL[leg.kind] ?? leg.kind}
                  </span>
                </span>
                <span className="text-gray-400">
                  {leg.srcSymbol} → {leg.destSymbol}
                  <span className="text-emerald-400 ml-2">
                    ${leg.amountUsdc.toFixed(2)}
                  </span>
                </span>
              </li>
            ))}
          </ol>

          <div className="bg-black/40 border border-white/5 p-3 text-xs font-mono mb-4">
            <div className="flex justify-between text-gray-400">
              <span>Paymaster (USDC gas)</span>
              <span className="text-emerald-300">
                ≈ ${estimatedFeeUsdc.toFixed(4)} USDC
              </span>
            </div>
            <div className="flex justify-between text-gray-400 mt-1">
              <span>Total amount routed</span>
              <span className="text-white">
                $
                {plan.legs.reduce((acc, l) => acc + l.amountUsdc, 0).toFixed(2)}
              </span>
            </div>
            <div className="text-[10px] text-gray-500 mt-2">
              via{" "}
              {feeSource === "paymaster" ? "Circle Paymaster" : "plan estimate"}
              {feeFetchedAt && (
                <>
                  {" · "}
                  {formatRelativeSeconds(feeFetchedAt)}
                </>
              )}
            </div>
          </div>

          {error && (
            <p className="text-xs text-rose-300 font-mono mb-3" role="alert">
              {error}
            </p>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-white/10 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-300 hover:text-white border border-white/10"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleApprove}
            disabled={submitting}
            className={cn(
              "px-4 py-2 text-sm font-semibold border-2",
              "bg-emerald-500 text-black border-emerald-300",
              "hover:bg-emerald-400 transition-colors",
              "disabled:opacity-50 disabled:cursor-not-allowed",
            )}
          >
            {submitting ? "Submitting…" : "Approve & execute"}
          </button>
        </footer>
      </div>
    </div>
  );
}
