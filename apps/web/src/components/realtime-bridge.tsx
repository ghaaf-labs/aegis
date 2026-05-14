"use client";

import { useCallback } from "react";
import { useEventSource, defaultSseUrl } from "@/lib/sse";
import { usePortfolioStore } from "@/stores/portfolio";
import type { AgentDecision, MarketRegime, PriceTick, RegimeFlip } from "@/types";

/**
 * Bridges the SSE channel into the Zustand store.
 *
 * Mounted once near the top of the tree (see `providers.tsx`). All components
 * read state via `usePortfolioStore`; this bridge is the only SSE subscriber.
 * Centralizing the subscription keeps the connection count at one regardless
 * of how many cards listen for live data.
 */
export function RealtimeBridge() {
  const addDecision = usePortfolioStore((s) => s.addDecision);
  const setRegime = usePortfolioStore((s) => s.setRegime);
  const applyPriceTick = usePortfolioStore((s) => s.applyPriceTick);
  const setSseConnected = usePortfolioStore((s) => s.setSseConnected);

  const onPriceTick = useCallback(
    (data: PriceTick) => applyPriceTick(data),
    [applyPriceTick]
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
    [setRegime]
  );

  const onAgentDecision = useCallback(
    (data: AgentDecision) => addDecision(data),
    [addDecision]
  );

  const { connected } = useEventSource(defaultSseUrl(), {
    "price.tick": onPriceTick,
    "regime.flip": onRegimeFlip,
    "agent.decision": onAgentDecision,
  });

  // Reflect connection state into the store so the UI can show a status dot.
  // We don't subscribe in render — `useEventSource` only changes `connected`
  // on real transitions, so this effect is safe to fire from a callback.
  if (typeof window !== "undefined") {
    queueMicrotask(() => setSseConnected(connected));
  }

  return null;
}
