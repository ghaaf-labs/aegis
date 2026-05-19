"use client";

import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Compact health indicator for the SSE channel. Cyan when the stream is open,
 * dim when reconnecting or absent. Mounted in the dashboard header so users
 * can tell at a glance whether prices and agent events are flowing.
 */
export function LivePill() {
  const connected = usePortfolioStore((s) => s.sseConnected);

  return (
    <span
      className="inline-flex items-center gap-1.5 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest border border-cyan-500/30 text-accent-agent bg-cyan-500/5"
      aria-label={
        connected
          ? "Live data stream is connected"
          : "Live data stream is reconnecting"
      }
    >
      <span
        className={
          connected
            ? "w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"
            : "w-1.5 h-1.5 rounded-full bg-cyan-700"
        }
      />
      {connected ? "Live" : "Offline"}
    </span>
  );
}
