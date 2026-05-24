"use client";

import { motion } from "framer-motion";
import { FEATURES } from "@/components/landing/landing-data";

export function FeaturesGrid() {
  return (
    <section className="relative z-10 max-w-6xl mx-auto px-6 pb-24">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="text-center mb-12"
      >
        <h2 className="text-3xl font-bold font-mono mb-3">
          Intelligence at every layer
        </h2>
        <p className="text-text-lo max-w-xl mx-auto">
          Modular AI agent architecture — from signal ingestion to on-chain
          execution.
        </p>
      </motion.div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {FEATURES.map((feature, i) => (
          <motion.div
            key={feature.title}
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ delay: i * 0.05 }}
            className="p-5 border-brutal border-border-default bg-surface hover:bg-raised transition-colors group"
          >
            <div className="w-9 h-9 rounded-sharp flex items-center justify-center mb-3 border-brutal bg-accent-agent/10 border-accent-agent/20 group-hover:bg-accent-agent/20 transition-colors">
              <feature.icon className="w-4 h-4 text-accent-agent" />
            </div>
            <h3 className="font-semibold text-sm text-text-hi mb-1.5 font-mono">
              {feature.title}
            </h3>
            <p className="text-xs text-text-lo leading-relaxed">
              {feature.description}
            </p>
          </motion.div>
        ))}
      </div>
    </section>
  );
}
