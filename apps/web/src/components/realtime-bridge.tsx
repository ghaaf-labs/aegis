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
import { getToken } from "@/lib/api";

/**
 * Bridges the SSE channel into the Zustand store.
 *
 * Mounted once near the top of the tree (see `providers.tsx`). All components
 * read state via `usePortfolioStore`; this bridge is the only SSE subscriber.
 * Centralizing the subscription keeps the connection count at one regardless
 * of how many cards listen for live data.
 *
 * `/sse` is authenticated — the EventSource only opens once a JWT is in
 * localStorage. On the public landing page the hook stays dormant.
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

  // The EventSource API doesn't support custom headers, so we put the token
  // in a query param. The handler in the server-side router could also read
  // it from a cookie — left as future hardening.
  const token = typeof window !== "undefined" ? getToken() : null;
  const enabled = !!token;
  const url = `${defaultSseUrl()}${token ? `?token=${encodeURIComponent(token)}` : ""}`;

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
      setPerChain(data.perChain ?? {}, data.perChainEurc ?? {});
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
