import * as React from "react";
import { cn } from "../utils";

/**
 * Cyan-bordered chip showing an OpenRouter model slug next to an agent
 * decision. Reads as "this model produced this output."
 */
export function ModelBadge({
  model,
  className,
}: {
  model: string;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-sharp",
        "font-mono text-[10px] border border-accent-agent/40 text-accent-agent bg-accent-agent/5",
        className,
      )}
      title={`Model: ${model}`}
    >
      <span className="opacity-70">⟁</span>
      {model}
    </span>
  );
}

/**
 * Chain pill — ARC / BASE / AVAX. Distinct stripe per chain.
 */
export function ChainBadge({
  chain,
  className,
}: {
  chain: "ARC" | "BASE" | "AVAX";
  className?: string;
}) {
  const stripe = {
    ARC: "border-accent-pnl/60",
    BASE: "border-accent-agent/60",
    AVAX: "border-risk/60",
  }[chain];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-sharp",
        "font-mono text-[10px] border bg-raised text-text-hi",
        stripe,
        className,
      )}
    >
      {chain}
    </span>
  );
}

/**
 * Free-form "via X · Ns ago" trailing note shown under any fetched value.
 * Mandatory per the trust-signals rule in docs/04-design-system.md.
 */
export function ProvenanceLine({
  source,
  freshness,
  className,
}: {
  source: string;
  freshness?: string;
  className?: string;
}) {
  return (
    <span className={cn("text-[10px] text-text-mut font-mono", className)}>
      via {source}
      {freshness ? ` · ${freshness}` : null}
    </span>
  );
}

/**
 * USDC fee preview surfaced in every approval modal. Always names the
 * paymaster as the source of truth.
 */
export function FeePreview({
  feeUsdc,
  className,
}: {
  feeUsdc: number;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex items-baseline gap-2 font-mono text-xs text-text-default",
        className,
      )}
    >
      <span className="text-text-lo">Fee</span>
      <span className="text-text-hi tabular-nums">
        ${feeUsdc.toFixed(feeUsdc >= 0.01 ? 4 : 6)} USDC
      </span>
      <ProvenanceLine source="Circle Paymaster" />
    </div>
  );
}
