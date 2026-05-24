"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { usePortfolioStore } from "@/stores/portfolio";
import { dashboardDestination } from "./dashboard-routing";
import { DashboardSkeleton } from "./dashboard-loading";

/**
 * Bare /dashboard URL — forwards to the most recently loaded portfolio.
 * New signups (no portfolio yet) are sent through onboarding to set their
 * goal + target allocation.
 */
export default function DashboardIndex() {
  const router = useRouter();
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const target = dashboardDestination(portfolios, activePortfolioId);
  const destination = target ? `/dashboard/${target.id}` : "/onboarding";

  useEffect(() => {
    if (!portfoliosLoaded) return;
    router.replace(destination);
    const fallback = window.setTimeout(() => {
      if (window.location.pathname === "/dashboard") {
        window.location.replace(destination);
      }
    }, 600);
    return () => window.clearTimeout(fallback);
  }, [destination, portfoliosLoaded, router]);

  if (!portfoliosLoaded || target) return <DashboardSkeleton />;

  return (
    <div className="min-h-[50vh] flex items-center justify-center">
      <div className="border-brutal border-border-default bg-raised p-6 text-center max-w-sm">
        <h1 className="text-sm font-semibold font-mono text-text-hi">
          Finish portfolio setup
        </h1>
        <p className="mt-2 text-xs font-mono text-text-lo leading-relaxed">
          Choose a goal and target mix before reviewing the dashboard.
        </p>
        <Link
          href={destination}
          className="mt-4 inline-flex px-3 py-1.5 border-2 border-accent-pnl bg-accent-pnl text-black text-xs font-semibold"
        >
          Continue setup
        </Link>
      </div>
    </div>
  );
}
