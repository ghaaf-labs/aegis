"use client";

import { useEffect } from "react";
import { usePathname } from "next/navigation";
import {
  agentApi,
  gatewayApi,
  marketApi,
  portfolioApi,
  walletApi,
} from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Hydrates the Zustand store from the API on app-shell mount:
 *  • `/portfolios`         — list of portfolios (no allocations)
 *  • `/portfolios/:id`     — detail with allocations, re-fetched whenever
 *                            the active portfolio changes
 *  • `/auth/session`       — restores account + wallet info after page reload
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
  const setMarketSnapshot = usePortfolioStore((s) => s.setMarketSnapshot);
  const setDecisions = usePortfolioStore((s) => s.setDecisions);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const setGatewayBalanceStatus = usePortfolioStore(
    (s) => s.setGatewayBalanceStatus,
  );
  const patchPortfolio = usePortfolioStore((s) => s.patchPortfolio);
  const activePortfolioId = usePortfolioStore((s) => s.activePortfolioId);
  const pathname = usePathname();
  const isExplore = pathname?.startsWith("/explore") ?? false;

  useEffect(() => {
    if (isExplore) {
      setPortfoliosLoaded(true);
      return;
    }
    let alive = true;
    walletApi
      .session()
      .then((session) => {
        if (!alive) return;
        if (!session.wallet) {
          setWallet(null);
          setPortfolios([]);
          setPortfoliosLoaded(true);
          return;
        }
        setWallet(session.wallet);
        return portfolioApi
          .list()
          .then((portfolios) => {
            if (alive) setPortfolios(portfolios);
          })
          .catch(() => {
            if (alive) setPortfoliosLoaded(true);
          });
      })
      .catch(() => {
        if (!alive) return;
        setWallet(null);
        setPortfolios([]);
        setPortfoliosLoaded(true);
      });
    // Market snapshot drives MarketOverview, AssetTable prices, etc. SSE only
    // emits per-tick deltas — without the initial snapshot the panels render
    // as loading skeletons forever.
    marketApi
      .snapshot()
      .then(setMarketSnapshot)
      .catch(() => {
        /* upstream provider may rate-limit — panels degrade to skeleton */
      });
    return () => {
      alive = false;
    };
  }, [
    isExplore,
    setPortfolios,
    setPortfoliosLoaded,
    setWallet,
    setMarketSnapshot,
  ]);

  // Whenever the active portfolio changes, fetch detail + merge allocations.
  useEffect(() => {
    if (!activePortfolioId || isExplore) return;
    portfolioApi
      .get(activePortfolioId)
      .then((p) => patchPortfolio(activePortfolioId, p))
      .catch((e) => {
        // 401 means session expired — handled by other paths. Anything else
        // surfaces as a debug log; the panels gracefully degrade to empty.
        console.warn("portfolio detail fetch failed", e);
      });
    // Hydrate the AI Reasoning feed. Without this the dashboard says "No
    // decisions yet" even when the agent has run dozens of times — SSE only
    // ever delivers *new* decisions, never history.
    agentApi
      .decisions(activePortfolioId)
      .then(setDecisions)
      .catch((e) => console.warn("agent decisions fetch failed", e));
    setGatewayBalanceStatus("loading");
    gatewayApi
      .balance()
      .then((b) => {
        setUnifiedUsdc(b.unifiedUsdc);
        setUnifiedEurc(b.unifiedEurc);
        setPerChain(b.perChain ?? {}, b.perChainEurc ?? {});
        setGatewayBalanceStatus("ready");
      })
      .catch((e) => {
        setGatewayBalanceStatus("error", gatewayBalanceError(e));
        console.warn("gateway balance fetch failed", e);
      });
  }, [
    activePortfolioId,
    isExplore,
    patchPortfolio,
    pathname,
    setDecisions,
    setGatewayBalanceStatus,
    setPerChain,
    setUnifiedEurc,
    setUnifiedUsdc,
  ]);

  return null;
}

function gatewayBalanceError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("401")) {
    return "Session expired before the balance check finished.";
  }
  if (message.toLowerCase().includes("gateway")) {
    return "Wallet balance check failed.";
  }
  return "Wallet balance is unavailable.";
}
