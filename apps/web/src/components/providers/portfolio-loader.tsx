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
  const setPortfoliosError = usePortfolioStore((s) => s.setPortfoliosError);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setMarketSnapshot = usePortfolioStore((s) => s.setMarketSnapshot);
  const setMarketSnapshotStatus = usePortfolioStore(
    (s) => s.setMarketSnapshotStatus,
  );
  const setDecisions = usePortfolioStore((s) => s.setDecisions);
  const setDecisionsStatus = usePortfolioStore((s) => s.setDecisionsStatus);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const setGatewayBalanceStatus = usePortfolioStore(
    (s) => s.setGatewayBalanceStatus,
  );
  const setActivePortfolioDetailStatus = usePortfolioStore(
    (s) => s.setActivePortfolioDetailStatus,
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
            // Distinguish a failed fetch from "genuinely no portfolios" so the
            // auth-gate shows a retry instead of booting an existing user into
            // the create-portfolio wizard.
            if (alive) {
              setPortfoliosError(true);
              setPortfoliosLoaded(true);
            }
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
    setMarketSnapshotStatus("loading");
    marketApi
      .snapshot()
      .then((snapshot) => {
        if (!alive) return;
        setMarketSnapshot(snapshot);
      })
      .catch(() => {
        if (alive) setMarketSnapshotStatus("error");
      });
    return () => {
      alive = false;
    };
  }, [
    isExplore,
    setPortfolios,
    setPortfoliosLoaded,
    setPortfoliosError,
    setWallet,
    setMarketSnapshot,
    setMarketSnapshotStatus,
  ]);

  // Whenever the active portfolio changes, fetch detail + merge allocations.
  useEffect(() => {
    if (!activePortfolioId || isExplore) return;
    let alive = true;
    setActivePortfolioDetailStatus(activePortfolioId, "loading");
    setDecisionsStatus(activePortfolioId, "loading");
    portfolioApi
      .get(activePortfolioId)
      .then((p) => {
        if (!alive) return;
        patchPortfolio(activePortfolioId, p);
        setActivePortfolioDetailStatus(activePortfolioId, "ready");
      })
      .catch((e) => {
        // 401 means session expired — handled by other paths. Anything else
        // surfaces as a debug log; the panels gracefully degrade to empty.
        if (alive) setActivePortfolioDetailStatus(activePortfolioId, "error");
        console.warn("portfolio detail fetch failed", e);
      });
    // Hydrate the AI Reasoning feed. Without this the dashboard says "No
    // decisions yet" even when the agent has run dozens of times — SSE only
    // ever delivers *new* decisions, never history.
    agentApi
      .decisions(activePortfolioId)
      .then((decisions) => {
        if (!alive) return;
        setDecisions(decisions);
        setDecisionsStatus(activePortfolioId, "ready");
      })
      .catch((e) => {
        if (alive) setDecisionsStatus(activePortfolioId, "error");
        console.warn("agent decisions fetch failed", e);
      });
    setGatewayBalanceStatus("loading");
    gatewayApi
      .balance()
      .then((b) => {
        if (!alive) return;
        setUnifiedUsdc(b.unifiedUsdc);
        setUnifiedEurc(b.unifiedEurc);
        setPerChain(
          b.perChain ?? {},
          b.perChainEurc ?? {},
          undefined,
          b.tokenBalancesByChain ?? {},
        );
        setGatewayBalanceStatus("ready");
      })
      .catch((e) => {
        if (!alive) return;
        setGatewayBalanceStatus("error", gatewayBalanceError(e));
        console.warn("gateway balance fetch failed", e);
      });
    return () => {
      alive = false;
    };
  }, [
    activePortfolioId,
    isExplore,
    patchPortfolio,
    setActivePortfolioDetailStatus,
    setDecisions,
    setDecisionsStatus,
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
