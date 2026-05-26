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
              className="flex justify-between gap-3 text-xs font-mono border border-white/5 p-2"
            >
              <span className="min-w-0">
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
                <LegMeta leg={leg} />
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

function formatTokenFloor(value: number) {
  return value >= 1 ? value.toFixed(4) : value.toFixed(8);
}

function LegMeta({ leg }: { leg: RebalancePlanResponse["legs"][number] }) {
  const deps = leg.deps ?? [];
  const minOutText = leg.minOut == null ? null : formatTokenFloor(leg.minOut);
  const hasMinOut = minOutText != null;
  const hasDeps = deps.length > 0;

  if (!hasMinOut && !hasDeps) {
    return null;
  }

  return (
    <span className="mt-1 block text-text-mut">
      {hasMinOut && <span>min out {minOutText}</span>}
      {hasMinOut && hasDeps && <span className="mx-1">·</span>}
      {hasDeps && <span>after {deps.map((dep) => dep + 1).join(", ")}</span>}
    </span>
  );
}
