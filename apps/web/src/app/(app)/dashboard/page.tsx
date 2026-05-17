"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Bare /dashboard URL — redirects to the active portfolio's dashboard, or
 * onboarding if the user has no portfolios yet.
 *
 * Waits for portfoliosLoaded before redirecting so a page refresh doesn't
 * send authenticated users to /onboarding before the API responds.
 */
export default function DashboardIndex() {
  const router = useRouter();
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const active = usePortfolioStore((s) => s.activePortfolioId);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);

  useEffect(() => {
    if (!portfoliosLoaded) return;
    const target = active ?? portfolios[0]?.id;
    if (target) {
      router.replace(`/dashboard/${target}`);
    } else {
      router.replace("/onboarding");
    }
  }, [router, active, portfolios, portfoliosLoaded]);

  return null;
}
