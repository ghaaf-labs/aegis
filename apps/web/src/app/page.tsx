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
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

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
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center">
            <Shield className="w-4 h-4 text-white" />
          </div>
          <span className="font-bold text-lg tracking-tight">Aegis</span>
        </div>
        <div className="flex items-center gap-3">
          <Link href="/explore">
            <Button
              variant="ghost"
              size="sm"
              className="text-gray-400 hover:text-white"
            >
              Explore demo
            </Button>
          </Link>
          <Link href="/signup">
            <Button
              size="sm"
              className="bg-blue-600 hover:bg-blue-500 text-white"
            >
              Get started
            </Button>
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
          <Badge className="mb-6 bg-blue-500/10 text-blue-400 border-blue-500/20 hover:bg-blue-500/20">
            <span className="mr-1.5 h-1.5 w-1.5 rounded-full bg-blue-400 inline-block animate-pulse" />
            AI Agent Active
          </Badge>
        </motion.div>

        <motion.h1
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          custom={1}
          className="text-6xl md:text-7xl font-bold tracking-tight mb-6 leading-[1.05]"
        >
          Your crypto portfolio,{" "}
          <span className="bg-clip-text text-transparent bg-gradient-to-r from-blue-400 via-violet-400 to-cyan-400">
            managed by AI
          </span>
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
            <Button
              size="lg"
              className="bg-blue-600 hover:bg-blue-500 text-white h-12 px-8 text-base font-medium"
            >
              Start for free
              <ArrowRight className="ml-2 w-4 h-4" />
            </Button>
          </Link>
          <Link href="/explore">
            <Button
              variant="outline"
              size="lg"
              className="h-12 px-8 text-base border-white/10 text-gray-300 hover:bg-white/5"
            >
              View demo
            </Button>
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
        <div className="relative rounded-2xl overflow-hidden border border-white/10 bg-gray-950/80 backdrop-blur-sm shadow-2xl shadow-black/50">
          <div className="flex items-center gap-1.5 px-4 py-3 border-b border-white/5">
            <div className="w-3 h-3 rounded-full bg-red-500/60" />
            <div className="w-3 h-3 rounded-full bg-yellow-500/60" />
            <div className="w-3 h-3 rounded-full bg-green-500/60" />
            <span className="ml-3 text-xs text-gray-500 font-mono">
              aegis.app/dashboard
            </span>
          </div>
          <div className="p-6 grid grid-cols-3 gap-4 min-h-[280px]">
            <div className="col-span-2 space-y-4">
              <div className="h-8 w-48 rounded-md bg-white/5 shimmer" />
              <div className="grid grid-cols-3 gap-3">
                {[...Array(3)].map((_, i) => (
                  <div key={i} className="h-24 rounded-xl bg-white/5 shimmer" />
                ))}
              </div>
              <div className="h-32 rounded-xl bg-white/5 shimmer" />
            </div>
            <div className="space-y-3">
              <div className="h-8 w-32 rounded-md bg-white/5 shimmer" />
              {[...Array(4)].map((_, i) => (
                <div
                  key={i}
                  className="h-16 rounded-xl bg-white/5 shimmer"
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
              className="p-6 rounded-2xl border border-white/8 bg-white/3 hover:bg-white/5 transition-colors group"
            >
              <div className="w-10 h-10 rounded-xl bg-blue-500/10 flex items-center justify-center mb-4 group-hover:bg-blue-500/20 transition-colors">
                <feature.icon className="w-5 h-5 text-blue-400" />
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
          className="p-12 rounded-3xl border border-white/10 bg-gradient-to-b from-blue-600/10 to-violet-600/5"
        >
          <h2 className="text-4xl font-bold mb-4">
            Ready to let AI manage your portfolio?
          </h2>
          <p className="text-gray-400 mb-8">
            Set your risk tolerance. Connect your portfolio. Let Aegis do the
            rest.
          </p>
          <Link href="/signup">
            <Button
              size="lg"
              className="bg-blue-600 hover:bg-blue-500 text-white h-12 px-10 text-base"
            >
              Get started for free
              <ArrowRight className="ml-2 w-4 h-4" />
            </Button>
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
