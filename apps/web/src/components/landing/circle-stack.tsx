"use client";

import { motion } from "framer-motion";
import { CIRCLE_STACK } from "@/components/landing/landing-data";

export function CircleStack() {
  return (
    <section className="relative z-10 max-w-6xl mx-auto px-6 pb-16">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="text-center mb-12"
      >
        <h2 className="text-3xl font-bold font-mono mb-3">
          Built entirely on{" "}
          <span className="text-accent-pnl">Circle&apos;s stack</span>
        </h2>
        <p className="text-text-lo max-w-xl mx-auto">
          Six Circle APIs. Every layer of the product — wallets, cross-chain,
          yield, gas, fees — runs on Circle infrastructure.
        </p>
      </motion.div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {CIRCLE_STACK.map((api, i) => (
          <motion.div
            key={api.name}
            initial={{ opacity: 0, y: 16 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ delay: i * 0.07 }}
            className="border-brutal border-border-default bg-surface p-5 space-y-2"
          >
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold font-mono text-accent-agent">
                {api.name}
              </span>
              <span className="text-[10px] font-mono text-text-mut border border-border-default px-1.5 py-0.5 rounded-sharp">
                {api.sub}
              </span>
            </div>
            <p className="text-xs text-text-lo leading-relaxed">{api.desc}</p>
          </motion.div>
        ))}
      </div>
    </section>
  );
}
