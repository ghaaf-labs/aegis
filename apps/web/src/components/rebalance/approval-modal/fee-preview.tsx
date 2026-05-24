import type { RebalancePlanResponse } from "@/lib/api";
import { formatRelativeSeconds } from "./helpers";

export function FeePreview({
  plan,
  routedUsdc,
  estimatedFeeUsdc,
  feeSource,
  feeFetchedAt,
}: {
  plan: RebalancePlanResponse;
  routedUsdc: number;
  estimatedFeeUsdc: number;
  feeSource: "plan" | "paymaster";
  feeFetchedAt?: Date | null;
}) {
  return (
    <div className="bg-black/40 border border-white/5 p-3 text-xs font-mono mb-4">
      <p className="mb-2 text-[10px] uppercase tracking-wider text-text-mut">
        Cost estimate
      </p>
      <div className="flex justify-between text-text-lo">
        <span>Plan amount</span>
        <span className="text-text-hi">${routedUsdc.toFixed(2)}</span>
      </div>
      <div className="mt-1 flex justify-between text-text-lo">
        <span title="Indicative gas estimate — not a binding quote. Actual settlement fee is published in the trace once the leg confirms.">
          Gas
        </span>
        <span className="text-accent-pnl">
          ≈ ${estimatedFeeUsdc.toFixed(4)} USDC
        </span>
      </div>
      <div className="text-[10px] text-text-mut mt-2">
        via {feeSource === "paymaster" ? "Circle Paymaster" : "plan estimate"}
        {feeFetchedAt && (
          <>
            {" · "}
            {formatRelativeSeconds(feeFetchedAt)}
          </>
        )}
      </div>

      {plan && plan.legs && plan.legs.length > 0 && (
        <div className="mt-3 text-sm flex justify-between border-t border-white/10 pt-2">
          <span className="text-warn">Aegis fee</span>
          <span className="font-mono text-warn">
            ≈ ${(routedUsdc * 0.0025).toFixed(4)} USDC
          </span>
        </div>
      )}

      {plan && plan.legs && plan.legs.length > 0 && (
        <div className="mt-2 pt-2 border-t border-white/10 flex justify-between text-sm font-semibold">
          <span className="text-text-hi">Estimated total</span>
          <span className="font-mono text-text-hi">
            ≈ ${(estimatedFeeUsdc + routedUsdc * 0.0025).toFixed(4)} USDC
          </span>
        </div>
      )}
    </div>
  );
}
