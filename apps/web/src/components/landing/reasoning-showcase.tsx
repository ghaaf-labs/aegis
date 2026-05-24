"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import { ExternalLink } from "lucide-react";

export function ReasoningShowcase() {
  return (
    <section className="relative z-10 max-w-4xl mx-auto px-6 pb-24">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="border-brutal border-border-default bg-surface p-8 space-y-6"
      >
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-bold font-mono text-text-hi">
            Transparent reasoning
          </h2>
          <Link
            href="/explore"
            className="text-xs font-mono text-accent-agent hover:underline flex items-center gap-1"
          >
            Explore demo portfolios <ExternalLink className="w-3 h-3" />
          </Link>
        </div>

        <div className="space-y-4">
          {/* Strategist */}
          <div className="border border-border-default bg-raised p-4 space-y-2">
            <div className="flex items-center gap-2">
              <span className="text-[10px] font-mono px-1.5 py-0.5 border border-accent-agent/30 text-accent-agent rounded-sharp">
                deepseek-v4-flash
              </span>
              <span className="text-[10px] font-mono text-text-mut uppercase tracking-widest">
                strategist
              </span>
              <div className="ml-auto flex gap-0.5">
                {[1, 2, 3, 4].map((d) => (
                  <span
                    key={d}
                    className="w-2 h-2 rounded-full bg-accent-agent"
                  />
                ))}
                <span className="w-2 h-2 rounded-full bg-border-default" />
              </div>
              <span className="text-[10px] font-mono text-text-mut">82%</span>
            </div>
            <p className="text-sm text-text-default leading-relaxed">
              Current regime is <em>consolidating</em> with a 14-day
              historical-vol of 0.4%. Arc liquidity is thin — recommending
              moving $800 USDC to Base via CCTP V2 for deeper swap routes at
              next rebalance. No tax impact — cost basis unchanged.
            </p>
          </div>

          {/* Critic */}
          <div className="border border-border-default bg-raised p-4 space-y-2">
            <div className="flex items-center gap-2">
              <span className="text-[10px] font-mono px-1.5 py-0.5 border border-text-mut/30 text-text-lo rounded-sharp">
                gpt-mini-latest
              </span>
              <span className="text-[10px] font-mono text-text-mut uppercase tracking-widest">
                critic
              </span>
            </div>
            <p className="text-sm text-text-default leading-relaxed">
              Agree. Cross-chain gas cost via Paymaster is $0.12 USDC —
              negligible. CCTP V2 mint latency is acceptable at current
              congestion. No objections; proposal is sound for the consolidating
              regime.
            </p>
          </div>

          <div className="flex items-center gap-2 text-xs font-mono text-text-mut">
            <span className="w-2 h-2 rounded-full bg-accent-agent" />
            Proposal queued for user approval · USDC fee: $0.12 via Nanopayments
          </div>
        </div>
      </motion.div>
    </section>
  );
}
