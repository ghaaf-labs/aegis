"use client";

import { useCallback, useEffect } from "react";
import { useEventSource, defaultSseUrl } from "@/lib/sse";
import { usePortfolioStore } from "@/stores/portfolio";
import type {
  AgentAbstained,
  AgentDecision,
  AgentToolInvoked,
  GatewayBalance,
  MarketRegime,
  PegAlert,
  PriceTick,
  RebalanceStatus,
  RegimeFlip,
  WalletInfo,
} from "@/types";

/**
 * Bridges the SSE channel into the Zustand store.
 *
 * Mounted once near the top of the tree (see `providers.tsx`). All components
 * read state via `usePortfolioStore`; this bridge is the only SSE subscriber.
 * Centralizing the subscription keeps the connection count at one regardless
 * of how many cards listen for live data.
 *
 * `/sse` is authenticated through the HttpOnly session cookie.
 */
export function RealtimeBridge() {
  const addDecision = usePortfolioStore((s) => s.addDecision);
  const setRegime = usePortfolioStore((s) => s.setRegime);
  const applyPriceTick = usePortfolioStore((s) => s.applyPriceTick);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSseConnected = usePortfolioStore((s) => s.setSseConnected);
  const pushToolInvocation = usePortfolioStore((s) => s.pushToolInvocation);
  const pushAbstain = usePortfolioStore((s) => s.pushAbstain);
  const applyRebalanceStatus = usePortfolioStore((s) => s.applyRebalanceStatus);
  const pushPegAlert = usePortfolioStore((s) => s.pushPegAlert);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);

  const enabled = sessionActive;
  const url = defaultSseUrl();

  const onPriceTick = useCallback(
    (data: PriceTick) => applyPriceTick(data),
    [applyPriceTick],
  );

  const onRegimeFlip = useCallback(
    (data: RegimeFlip) =>
      setRegime({
        current: data.to as MarketRegime,
        previous: (data.from as MarketRegime | null) ?? null,
        confidence: data.confidence,
        signals: data.signals,
        classifiedAt: data.classifiedAt,
      }),
    [setRegime],
  );

  const onAgentDecision = useCallback(
    (data: AgentDecision) => addDecision(data),
    [addDecision],
  );

  const onGatewayBalance = useCallback(
    (data: GatewayBalance) => {
      setUnifiedUsdc(data.unifiedUsdc);
      setUnifiedEurc(data.unifiedEurc);
      // Provenance uses the server's `observedAt` (when Circle was actually
      // queried), not the client receive time, so "refreshed Ns ago" reflects
      // data freshness rather than network/render latency.
      const observedAt = Date.parse(data.observedAt);
      setPerChain(
        data.perChain ?? {},
        data.perChainEurc ?? {},
        Number.isFinite(observedAt) ? observedAt : undefined,
      );
    },
    [setUnifiedUsdc, setUnifiedEurc, setPerChain],
  );

  const onWalletCreated = useCallback(
    (data: WalletInfo) => setWallet(data),
    [setWallet],
  );

  const onAgentToolInvoked = useCallback(
    (data: AgentToolInvoked) => pushToolInvocation(data),
    [pushToolInvocation],
  );

  const onAgentAbstained = useCallback(
    (data: AgentAbstained) => pushAbstain(data),
    [pushAbstain],
  );

  const onRebalanceStatus = useCallback(
    (data: RebalanceStatus) => applyRebalanceStatus(data),
    [applyRebalanceStatus],
  );

  const onPegAlert = useCallback(
    (data: PegAlert) => pushPegAlert(data),
    [pushPegAlert],
  );

  const { connected } = useEventSource(
    url,
    {
      "price.tick": onPriceTick,
      "regime.flip": onRegimeFlip,
      "agent.decision": onAgentDecision,
      "gateway.balance": onGatewayBalance,
      "wallet.created": onWalletCreated,
      "agent.tool.invoked": onAgentToolInvoked,
      "agent.abstained": onAgentAbstained,
      "rebalance.status": onRebalanceStatus,
      "peg.alert": onPegAlert,
    },
    { enabled },
  );

  useEffect(() => {
    setSseConnected(connected);
  }, [connected, setSseConnected]);

  return null;
}
