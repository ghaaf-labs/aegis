import * as React from "react";
import { cn } from "../utils";

type Tone = "raised" | "sunken";

export interface SkeletonProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Tailwind width class (e.g. `w-full`, `w-24`). */
  width?: string;
  /** Tailwind height class (e.g. `h-6`, `h-24`). */
  height?: string;
  /** Surface tone — `raised` matches BrutalCard's surface, `sunken` is darker. */
  tone?: Tone;
}

/**
 * Shared loading placeholder for the neo-brutalism surface language. Renders
 * a 2px-bordered rectangle with a subtle pulse. Drop-in for any region that
 * is still hydrating; pair with a sibling `ProvenanceLine` to keep the trust
 * signal visible while data lands.
 */
export function Skeleton({
  width = "w-full",
  height = "h-4",
  tone = "raised",
  className,
  ...rest
}: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        "shimmer border-[2px] border-border-default rounded-sharp",
        tone === "raised" ? "bg-raised" : "bg-surface",
        width,
        height,
        className,
      )}
      {...rest}
    />
  );
}
