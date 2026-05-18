"use client";

import { useEffect } from "react";
import { portfolioApi, walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Hydrates the Zustand store from the API on app-shell mount:
 *  • `/portfolios`         — list of portfolios (no allocations)
 *  • `/portfolios/:id`     — detail with allocations, re-fetched whenever
 *                            the active portfolio changes
 *  • `/auth/wallet/status` — restores wallet info after page reload
 *
 * Without this, the store initialises empty on every page load and:
 *  • the dashboard immediately redirects to /onboarding (no portfolios),
 *  • the header hides the Gateway USDC block (no wallet),
 *  • the AllocationChart / AssetTable show "No allocations yet" even
 *    though the DB has allocations (the list endpoint doesn't carry them).
 *
 * Renders nothing — purely a data-fetching side effect.
 */
export function PortfolioLoader() {
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const setPortfoliosLoaded = usePortfolioStore((s) => s.setPortfoliosLoaded);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const patchPortfolio = usePortfolioStore((s) => s.patchPortfolio);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);

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
    walletApi
      .status()
      .then((r) => {
        if (r.wallet) setWallet(r.wallet);
      })
      .catch(() => {
        /* unauthed or wallet pending — leave store null */
      });
  }, [setPortfolios, setPortfoliosLoaded, setWallet]);

  // Whenever the active portfolio changes, fetch detail + merge allocations.
  useEffect(() => {
    if (!activePortfolioId) return;
    portfolioApi
      .get(activePortfolioId)
      .then((p) => patchPortfolio(activePortfolioId, p))
      .catch((e) => {
        // 401 means session expired — handled by other paths. Anything else
        // surfaces as a debug log; the panels gracefully degrade to empty.
        console.warn("portfolio detail fetch failed", e);
      });
  }, [activePortfolioId, patchPortfolio]);

  return null;
}
