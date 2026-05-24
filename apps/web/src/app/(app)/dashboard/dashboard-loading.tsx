import { Skeleton } from "@aegis/ui";

export function DashboardSkeleton() {
  return (
    <div
      role="status"
      aria-live="polite"
      className="mx-auto w-full max-w-[1280px] space-y-5 md:space-y-6"
    >
      <span className="sr-only">Loading dashboard data</span>
      <div className="rounded-sharp border-brutal border-border-default bg-surface p-4 md:p-5">
        <Skeleton
          width="w-24"
          height="h-3"
          tone="sunken"
          className="border-accent-agent/20"
        />
        <Skeleton width="mt-5 w-full max-w-[520px]" height="h-10" />
        <Skeleton
          width="mt-4 w-full max-w-[680px]"
          height="h-4"
          tone="sunken"
        />
      </div>

      <div className="rounded-sharp border-brutal border-accent-pnl/50 bg-accent-pnl/5 p-4 md:p-5">
        <Skeleton
          width="w-24"
          height="h-3"
          tone="sunken"
          className="border-accent-pnl/20"
        />
        <Skeleton width="mt-5 w-full max-w-[420px]" height="h-9" />
        <div className="mt-5 grid gap-2 sm:grid-cols-3 lg:grid-cols-[1fr_1fr_1fr_minmax(240px,320px)]">
          <Skeleton height="h-16" tone="sunken" />
          <Skeleton height="h-16" tone="sunken" />
          <Skeleton height="h-16" tone="sunken" />
          <Skeleton height="h-16" className="border-accent-pnl/25" />
        </div>
      </div>

      <div className="grid grid-cols-1 items-start gap-4 md:grid-cols-2 2xl:grid-cols-3">
        <DashboardSkeletonCard />
        <DashboardSkeletonCard />
        <DashboardSkeletonCard />
      </div>
      <Skeleton height="h-44" tone="sunken" />
      <Skeleton height="h-80" tone="sunken" />
    </div>
  );
}

function DashboardSkeletonCard() {
  return (
    <div className="rounded-sharp border-brutal border-border-default bg-surface p-4">
      <Skeleton width="w-32" height="h-4" tone="sunken" />
      <Skeleton width="mt-8 w-48 max-w-full" height="h-9" />
      <div className="mt-5 space-y-2">
        <Skeleton height="h-9" tone="sunken" />
        <Skeleton height="h-9" tone="sunken" />
      </div>
      <Skeleton width="mt-12 w-44 max-w-full" height="h-3" tone="sunken" />
    </div>
  );
}
