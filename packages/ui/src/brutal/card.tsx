import * as React from "react";
import { cn } from "../utils";

type Variant = "default" | "raised";

export interface BrutalCardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: Variant;
  /** Hard offset shadow on hover. Default true. */
  shadow?: boolean;
}

/**
 * Neo-brutalism card. 2px solid border, hard offset shadow on hover,
 * no blur. Surfaces use the `surface` / `raised` palette tokens.
 */
export const BrutalCard = React.forwardRef<HTMLDivElement, BrutalCardProps>(
  ({ className, variant = "default", shadow = true, ...rest }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          "border-brutal border-border-default rounded-card",
          variant === "default" ? "bg-surface" : "bg-raised",
          "text-text-default",
          shadow && "hover:shadow-brutal transition-[box-shadow] duration-100",
          className,
        )}
        {...rest}
      />
    );
  },
);
BrutalCard.displayName = "BrutalCard";

export function BrutalCardHeader({
  className,
  ...rest
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "flex items-center justify-between px-4 py-3 border-b border-border-default",
        className,
      )}
      {...rest}
    />
  );
}

export function BrutalCardBody({
  className,
  ...rest
}: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("p-4", className)} {...rest} />;
}
