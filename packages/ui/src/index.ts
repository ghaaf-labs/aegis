// Shared UI primitives used across apps.
//
// Neo-brutalism dark, dual-accent system. See docs/04-design-system.md
// for the strict rules (green = money, cyan = agent, never mixed).
export { cn } from "./utils";

export {
  BrutalCard,
  BrutalCardHeader,
  BrutalCardBody,
  BrutalCardTitle,
} from "./brutal/card";
export type { BrutalCardProps } from "./brutal/card";

export { BrutalButton } from "./brutal/button";
export type { BrutalButtonProps, BrutalButtonVariant } from "./brutal/button";

export { BrutalPill } from "./brutal/pill";
export type { BrutalPillProps, PillTone } from "./brutal/pill";

export { BrutalBadge } from "./brutal/badge";
export type { BrutalBadgeProps } from "./brutal/badge";

export {
  ChainBadge,
  FeePreview,
  ModelBadge,
  ProvenanceLine,
} from "./brutal/badges";

export { Skeleton } from "./brutal/skeleton";
export type { SkeletonProps } from "./brutal/skeleton";
