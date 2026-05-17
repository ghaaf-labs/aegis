import * as React from "react";
import { cn } from "@/lib/utils";

// FE-SHAD-1 (phase 1): the four shadcn shims under components/ui/ now route
// through neo-brutalism design tokens (border-brutal, rounded-card / -sharp,
// bg-surface, bg-raised, accent-pnl / accent-agent) so every existing import
// inherits the neo-brutalism contract without a call-site migration. The
// wrapper files stay to keep call sites stable for this sprint; a phase-2
// follow-up should delete them and import @aegis/ui primitives directly.

const Card = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "border-brutal border-border-default rounded-card bg-surface text-text-default hover:shadow-brutal transition-[box-shadow] duration-100",
      className,
    )}
    {...props}
  />
));
Card.displayName = "Card";

const CardHeader = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "flex items-center justify-between px-4 py-3 border-b border-border-default",
      className,
    )}
    {...props}
  />
));
CardHeader.displayName = "CardHeader";

const CardTitle = React.forwardRef<
  HTMLParagraphElement,
  React.HTMLAttributes<HTMLHeadingElement>
>(({ className, ...props }, ref) => (
  <h3
    ref={ref}
    className={cn(
      "text-sm font-mono font-semibold tracking-tight text-text-hi",
      className,
    )}
    {...props}
  />
));
CardTitle.displayName = "CardTitle";

const CardContent = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("p-4", className)} {...props} />
));
CardContent.displayName = "CardContent";

export { Card, CardHeader, CardTitle, CardContent };
