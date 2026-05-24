"use client";

import { motion } from "framer-motion";
import { Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";

export function ProductTour() {
  return (
    <section className="relative z-10 max-w-6xl mx-auto px-6 pb-24">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="text-center mb-12"
      >
        <h2 className="text-3xl font-bold font-mono mb-3">
          Every surface, built for control
        </h2>
        <p className="text-text-lo max-w-xl mx-auto">
          From goal setting to on-chain execution — every screen is designed
          around one principle: you stay in control.
        </p>
      </motion.div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Strategies mockup */}
        <motion.div
          initial={{ opacity: 0, x: -20 }}
          whileInView={{ opacity: 1, x: 0 }}
          viewport={{ once: true }}
          className="border-brutal border-border-default bg-surface overflow-hidden"
        >
          <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-default bg-raised">
            <span className="text-[10px] font-mono text-text-mut">
              goal presets
            </span>
            <span className="text-[10px] font-mono text-text-mut">
              illustrative
            </span>
          </div>
          <div className="p-4 space-y-3">
            {[
              {
                name: "Conservative",
                alloc: "USDC 100% (executable today)",
                tag: "Low risk",
                color: "text-accent-agent",
              },
              {
                name: "Balanced",
                alloc: "USDC 70% · USYC 30% (USYC coming soon)",
                tag: "Medium risk",
                color: "text-text-default",
              },
              {
                name: "Growth",
                alloc: "USDC 50% · USYC 35% · EURC 15% (coming soon)",
                tag: "Higher yield",
                color: "text-accent-pnl",
              },
            ].map((s) => (
              <div
                key={s.name}
                className="flex items-center justify-between p-3 border border-border-default bg-raised/50"
              >
                <div>
                  <p className="text-xs font-mono font-semibold text-text-hi">
                    {s.name}
                  </p>
                  <p className="text-[10px] font-mono text-text-mut mt-0.5">
                    {s.alloc}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      "text-[10px] font-mono border px-1.5 py-0.5",
                      s.color === "text-accent-agent"
                        ? "border-accent-agent/30 text-accent-agent"
                        : s.color === "text-accent-pnl"
                          ? "border-accent-pnl/30 text-accent-pnl"
                          : "border-border-default text-text-lo",
                    )}
                  >
                    {s.tag}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </motion.div>

        {/* Agent Studio + Wallets stack */}
        <div className="flex flex-col gap-6">
          {/* Agent Studio mockup */}
          <motion.div
            initial={{ opacity: 0, x: 20 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            className="border-brutal border-border-default bg-surface overflow-hidden"
          >
            <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-default bg-raised">
              <span className="text-[10px] font-mono text-text-mut">
                agent-studio
              </span>
              <span className="flex items-center gap-1 text-[10px] font-mono text-accent-pnl">
                <span className="w-1.5 h-1.5 rounded-full bg-accent-pnl animate-pulse" />
                active
              </span>
            </div>
            <div className="p-4 grid grid-cols-2 gap-3">
              <div className="border border-border-default bg-raised p-3 space-y-1">
                <p className="text-[10px] font-mono text-text-lo uppercase tracking-widest">
                  Deployable surplus
                </p>
                <p className="text-lg font-mono font-bold text-text-hi tabular-nums">
                  $1,240
                </p>
                <p className="text-[10px] font-mono text-accent-agent">
                  above the USDC reserve
                </p>
              </div>
              <div className="border border-border-default bg-raised p-3 space-y-1">
                <p className="text-[10px] font-mono text-text-lo uppercase tracking-widest">
                  Invested
                </p>
                <p className="text-lg font-mono font-bold text-text-hi tabular-nums">
                  $22,940
                </p>
                <p className="text-[10px] font-mono text-text-mut">
                  across 3 assets
                </p>
              </div>
              <div
                className="col-span-2 flex items-center justify-center gap-2 border border-accent-agent/30 bg-accent-agent/5 py-2 text-xs font-mono text-accent-agent/60 cursor-default select-none"
                aria-label="Trigger analysis — available in the live app"
                title="Available in the live app"
              >
                <Sparkles className="w-3.5 h-3.5" />
                Trigger analysis
                <span className="text-[10px] font-mono text-text-mut border border-border-default px-1.5 py-0.5 rounded-sharp ml-1">
                  demo
                </span>
              </div>
            </div>
          </motion.div>

          {/* Wallets mockup */}
          <motion.div
            initial={{ opacity: 0, x: 20 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            transition={{ delay: 0.1 }}
            className="border-brutal border-border-default bg-surface overflow-hidden"
          >
            <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-default bg-raised">
              <span className="text-[10px] font-mono text-text-mut">
                wallets
              </span>
              <span className="text-[10px] font-mono text-text-mut">
                Arc · Base
              </span>
            </div>
            <div className="p-4 space-y-2">
              {[
                {
                  token: "USDC",
                  chain: "Arc",
                  balance: "$15,830",
                  route: "READY",
                  routeColor: "text-accent-pnl border-accent-pnl/30",
                },
                {
                  token: "USDC",
                  chain: "Base",
                  balance: "$8,350",
                  route: "READY",
                  routeColor: "text-accent-pnl border-accent-pnl/30",
                },
                {
                  token: "USYC",
                  chain: "Arc",
                  balance: "—",
                  route: "COMING SOON",
                  routeColor: "text-text-lo border-border-default",
                },
                {
                  token: "EURC",
                  chain: "Arc",
                  balance: "—",
                  route: "COMING SOON",
                  routeColor: "text-text-lo border-border-default",
                },
              ].map((row) => (
                <div
                  key={`${row.token}-${row.chain}`}
                  className="flex items-center justify-between py-1.5"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-mono font-semibold text-text-hi">
                      {row.token}
                    </span>
                    <span className="text-[10px] font-mono text-text-mut border border-border-default px-1">
                      {row.chain}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-mono text-text-default tabular-nums">
                      {row.balance}
                    </span>
                    <span
                      className={cn(
                        "text-[9px] font-mono border px-1.5 py-0.5",
                        row.routeColor,
                      )}
                    >
                      {row.route}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </motion.div>
        </div>
      </div>

      {/* Analytics mockup — full width */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="mt-6 border-brutal border-border-default bg-surface overflow-hidden"
      >
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-default bg-raised">
          <span className="text-[10px] font-mono text-text-mut">
            analytics · portfolio telemetry
          </span>
          <span className="text-[10px] font-mono text-accent-agent">
            consolidating regime
          </span>
        </div>
        <div className="p-4 grid grid-cols-2 md:grid-cols-4 gap-4">
          {[
            {
              label: "Net worth",
              value: "$24,180",
              sub: "↑ +2.4% today",
              subColor: "text-accent-pnl",
            },
            {
              label: "Decision quality",
              value: "82%",
              sub: "avg confidence · 14 decisions",
              subColor: "text-text-mut",
            },
            {
              label: "Target drift",
              value: "3.2%",
              sub: "USDC over-weight",
              subColor: "text-text-mut",
            },
            {
              label: "BTC dominance",
              value: "58.4%",
              sub: "risk-off signal",
              subColor: "text-text-mut",
            },
          ].map((s) => (
            <div
              key={s.label}
              className="border border-border-default bg-raised p-3"
            >
              <p className="text-[10px] font-mono text-text-lo uppercase tracking-widest mb-1">
                {s.label}
              </p>
              <p className="text-xl font-mono font-bold text-text-hi tabular-nums">
                {s.value}
              </p>
              <p className={cn("text-[10px] font-mono mt-0.5", s.subColor)}>
                {s.sub}
              </p>
            </div>
          ))}
        </div>
      </motion.div>
    </section>
  );
}
