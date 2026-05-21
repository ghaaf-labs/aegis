"use client";

import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Compact health indicator for the SSE channel. It describes the realtime
 * stream only; it does not imply real-money execution.
 */
export function LivePill() {
  const connected = usePortfolioStore((s) => s.sseConnected);

  return (
    <span
      className="inline-flex items-center gap-1.5 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest border border-cyan-500/30 text-accent-agent bg-cyan-500/5"
      aria-label={
        connected
          ? "Realtime stream is connected"
          : "Realtime stream is reconnecting"
      }
    >
      <span
        className={
          connected
            ? "w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"
            : "w-1.5 h-1.5 rounded-full bg-cyan-700"
        }
      />
      {connected ? "Stream" : "Offline"}
    </span>
  );
}
