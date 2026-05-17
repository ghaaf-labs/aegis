"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import {
  ArrowRight,
  Brain,
  Shield,
  TrendingUp,
  Zap,
  BarChart3,
  Bot,
  Trophy,
  FileSpreadsheet,
} from "lucide-react";
import { BrutalButton, BrutalPill } from "@aegis/ui";
import { PRICING_UI_ENABLED } from "@/lib/flags";

const fadeUp = {
  hidden: { opacity: 0, y: 24 },
  visible: (i = 0) => ({
    opacity: 1,
    y: 0,
    transition: { delay: i * 0.1, duration: 0.5, ease: "easeOut" },
  }),
};

const FEATURES = [
  {
    icon: Brain,
    title: "Multi-model agent loop",
    description:
      "Claude Opus 4.7 proposes, GPT-5 critiques, Haiku classifies regime — each routed via OpenRouter. Every decision surfaces the model slug it used.",
  },
  {
    icon: Shield,
    title: "USDC-native on Arc + Base",
    description:
      "Cross-chain rebalancing via CCTP V2 + Hooks. Gas paid in USDC by Circle Paymaster — never bridge ETH again. EURC sleeve via Arc StableFX.",
  },
  {
    icon: TrendingUp,
    title: "Realtime via SSE",
    description:
      "Server-sent events stream price ticks, regime flips, agent decisions, and rebalance legs to your browser the moment they happen.",
  },
  {
    icon: Zap,
    title: "Transparent reasoning",
    description:
      "Every decision ships with the strategist's prose, the critic's verdict, and a public diary — judge the agent on the receipts, not the demo.",
  },
  {
    icon: BarChart3,
    title: "Yield on idle USDC",
    description:
      "Park cash in USYC for the Hashnote treasury rate. The strategist factors current yield into every rebalance proposal.",
  },
  {
    icon: Bot,
    title: "You approve every move",
    description:
      "Set your goal once. The agent monitors 24/7 and proposes — you click Approve. Single modal, USDC fee preview, no surprises.",
  },
  {
    icon: FileSpreadsheet,
    title: "Tax-loss harvesting + 1099-DA",
    description:
      "The strategist spots open lots at a loss and proposes a harvest. Export a 1099-DA-ready CSV or share a time-limited link with your accountant.",
  },
  {
    icon: Trophy,
    title: "Trustability score + peg defense",
    description:
      "A rolling score grades the agent's decisions against realized outcomes. Peg-defense rules auto-propose rebalances the moment a stablecoin drifts.",
  },
];

export default function LandingPage() {
  return (
    <div className="min-h-screen bg-[#030712] text-white overflow-hidden">
      {/* Gradient orbs */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-[-20%] left-[10%] w-[600px] h-[600px] bg-blue-600/20 rounded-full blur-[120px]" />
        <div className="absolute top-[20%] right-[5%] w-[400px] h-[400px] bg-violet-600/20 rounded-full blur-[100px]" />
        <div className="absolute bottom-[10%] left-[30%] w-[500px] h-[500px] bg-cyan-600/10 rounded-full blur-[120px]" />
      </div>

      {/* Nav */}
      <nav className="relative z-10 flex items-center justify-between px-6 py-5 max-w-7xl mx-auto">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-bold text-lg tracking-tight text-text-hi">
            Aegis
          </span>
        </div>
        <div className="flex items-center gap-3">
          {PRICING_UI_ENABLED && (
            <Link
              href="/pricing"
              className="text-xs font-mono text-text-lo hover:text-text-hi"
            >
              Pricing
            </Link>
          )}
          <Link
            href="/explore"
            className="text-xs font-mono text-text-lo hover:text-text-hi"
          >
            Explore demo
          </Link>
          <Link
            href="/strategies"
            className="text-xs font-mono text-text-lo hover:text-text-hi"
          >
            Browse strategies
          </Link>
          <Link
            href="/leaderboard"
            className="text-xs font-mono text-text-lo hover:text-text-hi"
          >
            Leaderboard
          </Link>
          <Link href="/signup">
            <BrutalButton variant="pnl">Get started</BrutalButton>
          </Link>
        </div>
      </nav>

      {/* Hero */}
      <section className="relative z-10 pt-24 pb-32 px-6 text-center max-w-5xl mx-auto">
        <motion.div
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          custom={0}
        >
          <div className="mb-6 inline-flex">
            <BrutalPill tone="agent">
              <span className="mr-1.5 h-1.5 w-1.5 rounded-full bg-accent-agent inline-block animate-pulse" />
              AI Agent Active
            </BrutalPill>
          </div>
        </motion.div>

        <motion.h1
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          custom={1}
          className="text-6xl md:text-7xl font-bold tracking-tight mb-6 leading-[1.05]"
        >
          Your crypto portfolio,{" "}
          <span className="text-accent-agent">managed by AI</span>
        </motion.h1>

        <motion.p
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          custom={2}
          className="text-xl text-gray-400 max-w-2xl mx-auto mb-6 leading-relaxed"
        >
          Stablecoin-native portfolio agent. Set a goal, approve the moves — a
          multi-model AI executes on Arc + Base via Circle&apos;s stack, with
          USDC fees and a public reasoning diary.
        </motion.p>

        <motion.div
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          custom={3}
          className="flex flex-wrap items-center justify-center gap-2 mb-10 text-[11px] font-mono tracking-wide text-gray-500"
        >
          {[
            "Circle Wallets",
            "CCTP V2",
            "USYC",
            "Paymaster",
            "StableFX",
            "Nanopayments",
            "OpenRouter",
          ].map((label) => (
            <span
              key={label}
              className="px-2.5 py-1 rounded-full bg-white/[0.03] border border-white/10"
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
          <Link href="/signup">
            <BrutalButton variant="pnl">
              Start for free
              <ArrowRight className="ml-2 w-4 h-4" />
            </BrutalButton>
          </Link>
          <Link href="/explore">
            <BrutalButton variant="ghost">View demo</BrutalButton>
          </Link>
        </motion.div>
      </section>

      {/* Dashboard preview mockup */}
      <motion.section
        initial={{ opacity: 0, y: 40 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.5, duration: 0.7 }}
        className="relative z-10 max-w-6xl mx-auto px-6 mb-32"
      >
        <div className="border-brutal border-border-default bg-surface shadow-brutal">
          <div className="flex items-center gap-1.5 px-4 py-3 border-b border-white/5">
            <span className="text-xs text-text-mut font-mono">
              aegis.app/dashboard
            </span>
          </div>
          <div className="p-6 grid grid-cols-3 gap-4 min-h-[280px]">
            <div className="col-span-2 space-y-4">
              <div className="h-8 w-48 rounded-sharp bg-white/5 shimmer" />
              <div className="grid grid-cols-3 gap-3">
                {[...Array(3)].map((_, i) => (
                  <div
                    key={i}
                    className="h-24 rounded-sharp bg-white/5 shimmer"
                  />
                ))}
              </div>
              <div className="h-32 rounded-sharp bg-white/5 shimmer" />
            </div>
            <div className="space-y-3">
              <div className="h-8 w-32 rounded-sharp bg-white/5 shimmer" />
              {[...Array(4)].map((_, i) => (
                <div
                  key={i}
                  className="h-16 rounded-sharp bg-white/5 shimmer"
                  style={{ animationDelay: `${i * 0.3}s` }}
                />
              ))}
            </div>
          </div>
        </div>
      </motion.section>

      {/* Features */}
      <section className="relative z-10 max-w-6xl mx-auto px-6 pb-32">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          className="text-center mb-16"
        >
          <h2 className="text-4xl font-bold mb-4">
            Intelligence at every layer
          </h2>
          <p className="text-gray-400 text-lg max-w-xl mx-auto">
            Built with a modular AI agent architecture — from signal ingestion
            to portfolio execution.
          </p>
        </motion.div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {FEATURES.map((feature, i) => (
            <motion.div
              key={feature.title}
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ delay: i * 0.08 }}
              className="p-6 border-brutal border-border-default bg-surface hover:bg-raised transition-colors group"
            >
              <div className="w-10 h-10 rounded-sharp bg-accent-agent/10 flex items-center justify-center mb-4 group-hover:bg-accent-agent/20 transition-colors border-brutal border-accent-agent/20">
                <feature.icon className="w-5 h-5 text-accent-agent" />
              </div>
              <h3 className="font-semibold text-white mb-2">{feature.title}</h3>
              <p className="text-sm text-gray-400 leading-relaxed">
                {feature.description}
              </p>
            </motion.div>
          ))}
        </div>
      </section>

      {/* CTA */}
      <section className="relative z-10 max-w-3xl mx-auto px-6 pb-32 text-center">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          className="p-12 border-brutal border-border-default bg-surface"
        >
          <h2 className="text-4xl font-bold mb-4">
            Ready to let AI manage your portfolio?
          </h2>
          <p className="text-gray-400 mb-8">
            Set your risk tolerance. Connect your portfolio. Let Aegis do the
            rest.
          </p>
          <Link href="/signup">
            <BrutalButton variant="pnl">
              Get started for free
              <ArrowRight className="ml-2 w-4 h-4" />
            </BrutalButton>
          </Link>
        </motion.div>
      </section>

      {/* Footer */}
      <footer className="relative z-10 border-t border-white/5 px-6 py-8 text-center text-sm text-gray-600">
        <div className="flex items-center justify-center gap-2 mb-2">
          <Shield className="w-4 h-4" />
          <span className="font-semibold text-gray-500">Aegis</span>
        </div>
        <p>AI-powered crypto portfolio management. Built for the future.</p>
      </footer>
    </div>
  );
}
