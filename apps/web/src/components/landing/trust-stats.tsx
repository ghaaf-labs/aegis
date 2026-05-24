"use client";

import type { Traction } from "@/lib/api";
import { STAT_LABELS, tractionStats } from "@/components/landing/landing-data";

export function TrustStats({
  traction,
  statsLoaded,
}: {
  traction: Traction | null;
  statsLoaded: boolean;
}) {
  const stats = traction ? tractionStats(traction) : null;

  return (
    <div className="relative z-10 border-t border-b border-border-default bg-surface py-6 mb-16">
      <div className="max-w-4xl mx-auto px-6 space-y-4">
        {statsLoaded && !traction && (
          <p className="text-center text-xs font-mono text-text-mut">
            Live usage data is unavailable right now.
          </p>
        )}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-6">
          {(stats ?? STAT_LABELS).map((stat) => (
            <div key={stat.label} className="text-center">
              <p className="text-3xl font-mono font-bold text-text-hi tabular-nums">
                {"value" in stat ? (
                  stat.value
                ) : (
                  <span className="text-text-mut">—</span>
                )}
              </p>
              <p className="text-xs font-mono text-text-mut mt-1">
                {stat.label}
              </p>
            </div>
          ))}
        </div>
        {traction && (
          <p className="text-center text-[10px] font-mono text-text-mut">
            Live · via /api/traction
          </p>
        )}
      </div>
    </div>
  );
}
