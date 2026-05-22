"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { Loader2 } from "lucide-react";
import { usePortfolioStore } from "@/stores/portfolio";

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

  useEffect(() => {
    if (!portfoliosLoaded) return;
    const activeStillExists = portfolios.some(
      (p) => p.id === activePortfolioId,
    );
    const target = activeStillExists ? activePortfolioId : portfolios[0]?.id;
    if (target) {
      router.replace(`/dashboard/${target}`);
    } else {
      router.replace("/onboarding");
    }
  }, [router, activePortfolioId, portfolios, portfoliosLoaded]);

  if (!portfoliosLoaded) {
    return (
      <div className="min-h-[50vh] flex items-center justify-center">
        <div className="border-brutal border-border-default bg-raised p-6 text-center max-w-sm">
          <Loader2 className="w-5 h-5 animate-spin text-accent-agent mx-auto mb-3" />
          <h1 className="text-sm font-semibold font-mono text-text-hi">
            Restoring your workspace
          </h1>
          <p className="mt-2 text-xs font-mono text-text-lo leading-relaxed">
            Aegis is checking your wallet session and loading portfolios. If
            this does not move after a few seconds, sign in again with the same
            email.
          </p>
          <Link
            href="/login"
            className="mt-4 inline-flex px-3 py-1.5 border-2 border-accent-agent bg-accent-agent text-black text-xs font-semibold"
          >
            Sign in
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-[50vh] flex items-center justify-center">
      <div className="border-brutal border-border-default bg-raised p-6 text-center max-w-sm">
        <h1 className="text-sm font-semibold font-mono text-text-hi">
          Portfolio setup needed
        </h1>
        <p className="mt-2 text-xs font-mono text-text-lo leading-relaxed">
          Your wallet session is active, but no portfolio is attached yet.
          Create a target allocation to unlock the dashboard.
        </p>
        <Link
          href="/onboarding"
          className="mt-4 inline-flex px-3 py-1.5 border-2 border-accent-pnl bg-accent-pnl text-black text-xs font-semibold"
        >
          Create portfolio
        </Link>
      </div>
    </div>
  );
}
