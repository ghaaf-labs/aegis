"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import { ArrowRight } from "lucide-react";
import { BrutalPill } from "@aegis/ui";
import { fadeUp } from "@/components/landing/landing-data";

export function Hero() {
  return (
    <section className="relative z-10 pt-20 pb-24 px-6 text-center max-w-5xl mx-auto">
      <motion.div
        initial="hidden"
        animate="visible"
        variants={fadeUp}
        custom={0}
        className="mb-6 inline-flex"
      >
        <Link href="/leaderboard">
          <BrutalPill tone="agent">
            <span className="mr-1.5 h-1.5 w-1.5 rounded-full bg-accent-agent inline-block animate-pulse" />
            AI Agent Active · View leaderboard
          </BrutalPill>
        </Link>
      </motion.div>

      <motion.h1
        initial="hidden"
        animate="visible"
        variants={fadeUp}
        custom={1}
        className="text-5xl md:text-7xl font-bold tracking-tight mb-4 leading-[1.05] font-mono"
      >
        Set a goal. <span className="text-accent-agent">Agent proposes.</span>
        <br /> You approve.
      </motion.h1>

      <motion.p
        initial="hidden"
        animate="visible"
        variants={fadeUp}
        custom={2}
        className="text-lg text-text-lo max-w-2xl mx-auto mb-6 leading-relaxed"
      >
        Stablecoin-native portfolio management on Arc + Base. Every rebalance
        needs your sign-off — no black box, no surprises, full reasoning diary.
      </motion.p>

      <motion.div
        initial="hidden"
        animate="visible"
        variants={fadeUp}
        custom={3}
        className="flex flex-wrap items-center justify-center gap-2 mb-10 text-[11px] font-mono tracking-wide text-text-mut"
      >
        {[
          "Circle Wallets",
          "CCTP V2",
          "Paymaster",
          "Nanopayments",
          "OpenRouter",
        ].map((label) => (
          <span
            key={label}
            className="px-2.5 py-1 rounded-full bg-raised border border-border-default"
          >
            {label}
          </span>
        ))}
      </motion.div>

      <motion.div
        initial="hidden"
        animate="visible"
        variants={fadeUp}
        custom={4}
        className="flex items-center justify-center gap-4"
      >
        <Link
          href="/login"
          className="inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold border-brutal border-black rounded-sharp transition-[box-shadow,transform] duration-100 active:translate-y-px bg-accent-pnl text-black hover:shadow-brutal-sm"
        >
          Start for free
          <ArrowRight className="ml-2 w-4 h-4" />
        </Link>
        <Link
          href="/explore"
          className="inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold border-brutal border-black rounded-sharp transition-[box-shadow,transform] duration-100 active:translate-y-px bg-transparent text-text-default hover:text-text-hi hover:bg-raised"
        >
          View demo
        </Link>
      </motion.div>
    </section>
  );
}
