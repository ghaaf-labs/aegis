"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import { Trophy } from "lucide-react";
import { cn } from "@/lib/utils";
import type { LeaderboardEntry } from "@/lib/api";
import { LABEL_TONE, signedPct } from "@/components/landing/landing-data";

export function LeaderboardPreview({
  topPerformers,
  statsLoaded,
}: {
  topPerformers: LeaderboardEntry[];
  statsLoaded: boolean;
}) {
  return (
    <section className="relative z-10 max-w-3xl mx-auto px-6 pb-24">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="border-brutal border-border-default bg-surface"
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-border-default">
          <div className="flex items-center gap-2">
            <Trophy className="w-4 h-4 text-accent-pnl" />
            <span className="text-sm font-semibold font-mono text-text-hi">
              Top performers this week
            </span>
          </div>
          <Link
            href="/leaderboard"
            className="text-xs font-mono text-accent-agent hover:underline"
          >
            Full leaderboard →
          </Link>
        </div>
        {topPerformers.length === 0 ? (
          <div className="px-5 py-8 text-center space-y-2">
            <p className="text-xs font-mono text-text-lo">
              {statsLoaded
                ? "No ranked portfolios yet — be the first on the board."
                : "Loading top performers…"}
            </p>
            {statsLoaded && (
              <Link
                href="/login"
                className="inline-block text-xs font-mono text-accent-agent hover:underline"
              >
                Start for free →
              </Link>
            )}
          </div>
        ) : (
          <div className="divide-y divide-border-default">
            {topPerformers.map((entry, i) => {
              const returnTone =
                entry.avg7dReturn >= 0 ? "text-accent-pnl" : "text-risk";
              return (
                <div
                  key={entry.userId}
                  className="flex items-center gap-4 px-5 py-3"
                >
                  <span className="text-sm font-mono text-text-mut w-4 text-center">
                    {i + 1}
                  </span>
                  <Link
                    href={`/diary/${entry.handle}`}
                    className="text-xs font-mono text-text-default flex-1 truncate hover:text-accent-agent transition-colors"
                  >
                    <span className="opacity-70">0x</span>
                    {entry.handle}
                  </Link>
                  <span className="text-xs font-mono text-text-mut">
                    {entry.decisionsExecuted} decisions
                  </span>
                  <span
                    className={cn(
                      "text-sm font-mono font-semibold tabular-nums",
                      returnTone,
                    )}
                  >
                    {signedPct(entry.avg7dReturn)}
                  </span>
                  <span
                    className={cn(
                      "text-[10px] font-mono px-1.5 py-0.5 border rounded-sharp",
                      LABEL_TONE[entry.label],
                    )}
                  >
                    {entry.label}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </motion.div>
    </section>
  );
}
