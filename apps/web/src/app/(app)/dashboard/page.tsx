"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { Loader2 } from "lucide-react";
import { usePortfolioStore } from "@/stores/portfolio";
import { dashboardDestination } from "./dashboard-routing";

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

  if (!portfoliosLoaded) {
    return (
      <div className="min-h-[50vh] flex items-center justify-center">
        <div className="border-brutal border-border-default bg-raised p-6 text-center max-w-sm">
          <Loader2 className="w-5 h-5 animate-spin text-accent-agent mx-auto mb-3" />
          <h1 className="text-sm font-semibold font-mono text-text-hi">
            Restoring your workspace
          </h1>
          <p className="mt-2 text-xs font-mono text-text-lo leading-relaxed">
            Loading your portfolios. If this takes more than a few seconds, sign
            in again.
          </p>
          <Link
            href="/login"
            className="mt-4 inline-flex px-3 py-1.5 border-2 border-accent-agent bg-accent-agent text-black text-xs font-semibold"
          >
            Sign in again
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-[50vh] flex items-center justify-center">
      <div className="border-brutal border-border-default bg-raised p-6 text-center max-w-sm">
        {target && (
          <Loader2 className="w-5 h-5 animate-spin text-accent-agent mx-auto mb-3" />
        )}
        <h1 className="text-sm font-semibold font-mono text-text-hi">
          {target ? "Opening dashboard" : "Finish portfolio setup"}
        </h1>
        <p className="mt-2 text-xs font-mono text-text-lo leading-relaxed">
          {target
            ? `Taking you to ${target.name || "your portfolio"}.`
            : "Choose a goal and target mix before reviewing the dashboard."}
        </p>
        <Link
          href={destination}
          className="mt-4 inline-flex px-3 py-1.5 border-2 border-accent-pnl bg-accent-pnl text-black text-xs font-semibold"
        >
          {target ? "Open dashboard" : "Continue setup"}
        </Link>
      </div>
    </div>
  );
}
