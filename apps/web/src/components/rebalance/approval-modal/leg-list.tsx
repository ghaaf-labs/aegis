import { useState } from "react";
import { ChainBadge } from "@aegis/ui";
import type { RebalancePlanResponse } from "@/lib/api";
import { KIND_LABEL, legRouteText, toChainBadge } from "./helpers";

export function LegList({ plan }: { plan: RebalancePlanResponse }) {
  const [routeOpen, setRouteOpen] = useState(false);

  return (
    <div className="mb-4 border border-white/10 bg-black/20">
      <button
        type="button"
        onClick={() => setRouteOpen((v) => !v)}
        className="flex w-full items-center justify-between px-3 py-2 text-left text-xs font-mono text-text-hi hover:bg-white/5"
      >
        <span>Route details</span>
        <span className="text-text-mut">
          {plan.totalLegs} move{plan.totalLegs === 1 ? "" : "s"}{" "}
          {routeOpen ? "shown" : "hidden"}
        </span>
      </button>
      {routeOpen && (
        <ol className="space-y-2 border-t border-white/10 p-3">
          {plan.legs.map((leg) => (
            <li
              key={leg.legIndex}
              data-testid="leg-card"
              className="flex justify-between text-xs font-mono border border-white/5 p-2"
            >
              <span className="flex items-center gap-1.5">
                <span className="text-text-mut">
                  {String(leg.legIndex + 1).padStart(2, "0")}
                </span>
                <span className="text-text-hi">
                  {KIND_LABEL[leg.kind] ?? leg.kind}
                </span>
                {leg.srcChain && (
                  <ChainBadge chain={toChainBadge(leg.srcChain)} />
                )}
                {leg.destChain && leg.destChain !== leg.srcChain && (
                  <ChainBadge chain={toChainBadge(leg.destChain)} />
                )}
              </span>
              <span className="text-text-lo">
                {legRouteText(leg)}
                <span className="text-accent-pnl ml-2">
                  ${leg.amountUsdc.toFixed(2)}
                </span>
              </span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
