import * as React from "react";
import { cn } from "../utils";

export type BrutalButtonVariant =
  | "pnl"
  | "agent"
  | "neutral"
  | "ghost"
  | "danger";

export interface BrutalButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: BrutalButtonVariant;
}

const variantClass: Record<BrutalButtonVariant, string> = {
  // Green — money / approvals. Never in agent surfaces.
  pnl: "bg-accent-pnl text-black hover:shadow-brutal-sm",
  // Cyan — agent actions ("Ask agent", "Re-run analysis"). Never in PnL.
  agent: "bg-accent-agent text-black hover:shadow-brutal-sm",
  // Neutral primary.
  neutral: "bg-text-hi text-black hover:shadow-brutal-sm",
  ghost: "bg-transparent text-text-default hover:text-text-hi hover:bg-raised",
  danger: "bg-risk text-black hover:shadow-brutal-sm",
};

/**
 * Neo-brutalism button. Picks tone via `variant`; never mix tones with
 * conflicting semantic colors (e.g. don't put a cyan button on a PnL card).
 */
export const BrutalButton = React.forwardRef<
  HTMLButtonElement,
  BrutalButtonProps
>(({ className, variant = "neutral", ...rest }, ref) => {
  return (
    <button
      ref={ref}
      className={cn(
        "inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold",
        "border-brutal border-black rounded-sharp",
        "transition-[box-shadow,transform] duration-100 active:translate-y-px",
        "disabled:cursor-not-allowed disabled:opacity-45 disabled:shadow-none disabled:active:translate-y-0",
        variantClass[variant],
        className,
      )}
      {...rest}
    />
  );
});
BrutalButton.displayName = "BrutalButton";
