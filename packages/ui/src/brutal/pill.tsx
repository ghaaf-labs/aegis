import * as React from "react";
import { cn } from "../utils";

export type PillTone = "pnl" | "agent" | "neutral" | "warn" | "risk";

const toneClass: Record<PillTone, string> = {
  pnl: "bg-accent-pnl text-black",
  agent: "bg-accent-agent text-black",
  neutral: "bg-text-hi text-black",
  warn: "bg-warn text-black",
  risk: "bg-risk text-black",
};

export interface BrutalPillProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: PillTone;
}

/**
 * Solid colored block used for regime labels (RISK-ON / NEUTRAL / RISK-OFF)
 * and similar binary state pills. Always black text on the solid block —
 * never the inverse.
 */
export const BrutalPill = React.forwardRef<HTMLSpanElement, BrutalPillProps>(
  ({ className, tone = "neutral", children, ...rest }, ref) => {
    return (
      <span
        ref={ref}
        className={cn(
          "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-sharp",
          "font-mono text-[10px] font-semibold tracking-tight",
          toneClass[tone],
          className,
        )}
        {...rest}
      >
        {children}
      </span>
    );
  },
);
BrutalPill.displayName = "BrutalPill";
