"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Bare /dashboard URL — every Aegis user has exactly one portfolio, so this
 * just forwards to it. New signups (no portfolio yet) are sent through
 * onboarding to set their goal + target allocation.
 */
export default function DashboardIndex() {
  const router = useRouter();
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);

  useEffect(() => {
    if (!portfoliosLoaded) return;
    const target = portfolios[0]?.id;
    if (target) {
      router.replace(`/dashboard/${target}`);
    } else {
      router.replace("/onboarding");
    }
  }, [router, portfolios, portfoliosLoaded]);

  return null;
}
