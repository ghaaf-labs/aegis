import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../utils";

const badgeVariants = cva(
  "inline-flex items-center border-brutal rounded-sharp px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest transition-colors focus:outline-none focus:ring-2 focus:ring-cyan-400 focus:ring-offset-2",
  {
    variants: {
      variant: {
        default: "border-text-hi/40 bg-text-hi/10 text-text-hi",
        secondary: "border-border-default bg-raised text-text-default",
        destructive: "border-risk/40 bg-risk/10 text-risk",
        outline: "border-border-default bg-transparent text-text-default",
        success: "border-accent-pnl/40 bg-accent-pnl/10 text-accent-pnl",
        warning: "border-warn/40 bg-warn/10 text-warn",
        danger: "border-risk/40 bg-risk/10 text-risk",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface BrutalBadgeProps
  extends
    React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

/**
 * Subtle outlined status chip (tinted background + colored border/text). The
 * quieter counterpart to {@link BrutalPill}'s solid block — use for inline
 * labels like drift deltas, trigger kinds and model slugs.
 */
export function BrutalBadge({
  className,
  variant,
  ...props
}: BrutalBadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}
