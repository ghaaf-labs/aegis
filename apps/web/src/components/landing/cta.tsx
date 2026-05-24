"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import { ArrowRight } from "lucide-react";

export function Cta() {
  return (
    <section className="relative z-10 max-w-3xl mx-auto px-6 pb-24 text-center">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="p-12 border-brutal border-border-default bg-surface"
      >
        <h2 className="text-3xl font-bold font-mono mb-3">
          Ready to let AI manage your portfolio?
        </h2>
        <p className="text-text-lo mb-8 text-sm">
          Set your risk tolerance. Connect your portfolio. Every move needs your
          sign-off.
        </p>
        <Link
          href="/login"
          className="inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold border-brutal border-black rounded-sharp transition-[box-shadow,transform] duration-100 active:translate-y-px bg-accent-pnl text-black hover:shadow-brutal-sm"
        >
          Get started for free
          <ArrowRight className="ml-2 w-4 h-4" />
        </Link>
      </motion.div>
    </section>
  );
}
