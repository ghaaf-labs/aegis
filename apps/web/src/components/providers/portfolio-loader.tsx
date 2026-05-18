"use client";

import { useEffect } from "react";
import { portfolioApi, walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Fetches the current user's portfolios + wallet once on app-shell mount
 * and writes them into the Zustand store. Without this, the store
 * initialises empty on every page load and:
 *  • the dashboard immediately redirects to /onboarding (no portfolios),
 *  • the header hides the Gateway USDC block (no wallet),
 *  • the dashboard hides the faucet/balance UI (no wallet).
 *
 * Renders nothing — purely a data-fetching side effect.
 */
export function PortfolioLoader() {
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const setPortfoliosLoaded = usePortfolioStore((s) => s.setPortfoliosLoaded);
  const setWallet = usePortfolioStore((s) => s.setWallet);

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
    // Rehydrate the wallet from /auth/wallet/status — survives page reloads
    // where the Zustand store starts empty.
    walletApi
      .status()
      .then((r) => {
        if (r.wallet) setWallet(r.wallet);
      })
      .catch(() => {
        /* unauthed or wallet pending — leave store null */
      });
  }, [setPortfolios, setPortfoliosLoaded, setWallet]);

  return null;
}
