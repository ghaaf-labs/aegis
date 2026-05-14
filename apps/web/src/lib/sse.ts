"use client";

import { useEffect, useRef, useState } from "react";
import type { SseEventType, SseEventMap } from "@/types";

export interface UseEventSourceOptions {
  /**
   * If false, the connection is not opened. Useful to gate on auth state or
   * route hydration without unmounting the component.
   */
  enabled?: boolean;
  /**
   * Fallback reconnect delay when the server hasn't sent a `retry:` field.
   * Defaults to 3000ms.
   */
  reconnectDelayMs?: number;
}

export interface UseEventSourceResult {
  connected: boolean;
  /** True while the browser is in a reconnect backoff window. */
  reconnecting: boolean;
}

type Handlers = Partial<{
  [K in SseEventType]: (data: SseEventMap[K]) => void;
}>;

/**
 * Typed wrapper around the browser's `EventSource` for our SSE channel.
 *
 * Pass a partial map of event handlers keyed by the same `type` discriminator
 * the backend uses (e.g. `price.tick`, `agent.decision`). Returns connection
 * state for status indicators in the UI.
 */
export function useEventSource(
  url: string,
  handlers: Handlers,
  options: UseEventSourceOptions = {},
): UseEventSourceResult {
  const { enabled = true, reconnectDelayMs = 3000 } = options;
  const [connected, setConnected] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);

  // Hold the latest handlers in a ref so we don't tear down the EventSource
  // every time a parent re-renders with a fresh callback identity.
  const handlersRef = useRef<Handlers>(handlers);
  useEffect(() => {
    handlersRef.current = handlers;
  }, [handlers]);

  useEffect(() => {
    if (!enabled || typeof window === "undefined") return;

    let cancelled = false;
    let source: EventSource | null = null;
    let retryTimer: ReturnType<typeof setTimeout> | null = null;

    const open = () => {
      if (cancelled) return;
      source = new EventSource(url, { withCredentials: false });

      source.onopen = () => {
        if (cancelled) return;
        setConnected(true);
        setReconnecting(false);
      };

      source.onerror = () => {
        if (cancelled) return;
        setConnected(false);
        // EventSource auto-reconnects on transient failures, but some
        // proxies close the stream cleanly which results in `CLOSED`.
        // Force a manual reopen in that case.
        if (source && source.readyState === EventSource.CLOSED) {
          setReconnecting(true);
          source.close();
          source = null;
          retryTimer = setTimeout(open, reconnectDelayMs);
        }
      };

      // Wire every typed event by name. Unknown events are ignored.
      const eventTypes: SseEventType[] = [
        "price.tick",
        "regime.flip",
        "agent.decision",
        "rebalance.status",
        "gateway.balance",
      ];
      for (const type of eventTypes) {
        source.addEventListener(type, (event) => {
          try {
            const payload = JSON.parse((event as MessageEvent).data);
            const handler = handlersRef.current[type] as
              | ((data: unknown) => void)
              | undefined;
            handler?.(payload);
          } catch {
            // Malformed payloads are dropped; SSE has no replay so retrying
            // is pointless. The server's tracing logs catch this case.
          }
        });
      }
    };

    open();

    return () => {
      cancelled = true;
      if (retryTimer) clearTimeout(retryTimer);
      if (source) source.close();
      setConnected(false);
      setReconnecting(false);
    };
  }, [url, enabled, reconnectDelayMs]);

  return { connected, reconnecting };
}

/**
 * Default SSE URL: prefers `NEXT_PUBLIC_SSE_URL`, falls back to deriving from
 * `NEXT_PUBLIC_API_URL`, finally falls back to localhost. Server-rendered
 * pages get a safe default; the hook only opens the connection client-side.
 */
export function defaultSseUrl(): string {
  if (typeof process !== "undefined" && process.env?.NEXT_PUBLIC_SSE_URL) {
    return process.env.NEXT_PUBLIC_SSE_URL;
  }
  if (typeof process !== "undefined" && process.env?.NEXT_PUBLIC_API_URL) {
    return `${process.env.NEXT_PUBLIC_API_URL.replace(/\/$/, "")}/sse`;
  }
  return "http://localhost:8080/sse";
}
