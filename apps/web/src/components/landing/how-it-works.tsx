"use client";

import { motion } from "framer-motion";
import { ChevronRight } from "lucide-react";
import { HOW_IT_WORKS } from "@/components/landing/landing-data";

export function HowItWorks() {
  return (
    <section className="relative z-10 max-w-6xl mx-auto px-6 pb-24">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="text-center mb-12"
      >
        <h2 className="text-3xl font-bold font-mono mb-3">How it works</h2>
        <p className="text-text-lo max-w-xl mx-auto">
          Three steps. You stay in control. The agent does the analysis.
        </p>
      </motion.div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {HOW_IT_WORKS.map((step, i) => (
          <motion.div
            key={step.step}
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ delay: i * 0.1 }}
            className="border-brutal border-border-default bg-surface p-6 space-y-4"
          >
            <div className="flex items-center gap-3">
              <span className="text-3xl font-mono font-bold text-accent-agent/30">
                {step.step}
              </span>
              <h3 className="text-base font-semibold font-mono text-text-hi">
                {step.title}
              </h3>
            </div>
            <ul className="space-y-2">
              {step.items.map((item) => (
                <li
                  key={item}
                  className="flex items-start gap-2 text-xs font-mono text-text-lo"
                >
                  <ChevronRight className="w-3 h-3 text-accent-agent shrink-0 mt-0.5" />
                  {item}
                </li>
              ))}
            </ul>
          </motion.div>
        ))}
      </div>
    </section>
  );
}
