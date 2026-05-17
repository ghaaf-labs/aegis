"use client";

import { useEffect } from "react";
import { portfolioApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Fetches the current user's portfolios once on app-shell mount and writes
 * them into the Zustand store. Without this, the store initialises empty on
 * every page load and the dashboard immediately redirects to /onboarding.
 *
 * Renders nothing — purely a data-fetching side effect.
 */
export function PortfolioLoader() {
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const setPortfoliosLoaded = usePortfolioStore((s) => s.setPortfoliosLoaded);

  useEffect(() => {
    portfolioApi
      .list()
      .then(setPortfolios)
      .catch(() => {
        // Auth failures (401) are expected when the session has expired.
        // Mark as loaded so the dashboard can redirect to /login rather
        // than spinning forever.
        setPortfoliosLoaded(true);
      });
  }, [setPortfolios, setPortfoliosLoaded]);

  return null;
}
