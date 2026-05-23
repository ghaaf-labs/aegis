"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { motion } from "framer-motion";
import {
  ArrowRight,
  BarChart3,
  Bot,
  Brain,
  ChevronRight,
  ExternalLink,
  FileSpreadsheet,
  PieChart,
  Shield,
  Sparkles,
  Target,
  TrendingUp,
  Trophy,
  Wallet,
  X,
  Zap,
} from "lucide-react";
import { BrutalPill } from "@aegis/ui";
import { cn } from "@/lib/utils";
import {
  leaderboardApi,
  tractionApi,
  type LeaderboardEntry,
  type Traction,
} from "@/lib/api";
import { LandingHeader } from "@/components/layout/landing-header";
import { LandingFooter } from "@/components/layout/landing-footer";

const LEADERBOARD_PREVIEW_COUNT = 3;

/** Compact USD formatting for the trust-stats bar: $0 → "$0", 1_240 →
 *  "$1.2k", 142_000 → "$142k", 3_200_000 → "$3.2M". */
function formatCompactUsd(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "$0";
  if (value < 1_000) return `$${Math.round(value)}`;
  if (value < 1_000_000) {
    const k = value / 1_000;
    return `$${k < 10 ? k.toFixed(1).replace(/\.0$/, "") : Math.round(k)}k`;
  }
  const m = value / 1_000_000;
  return `$${m < 10 ? m.toFixed(1).replace(/\.0$/, "") : Math.round(m)}M`;
}

const STAT_LABELS = [
  { label: "portfolios" },
  { label: "agent decisions" },
  { label: "USDC managed" },
  { label: "chains" },
] as const;

function tractionStats(t: Traction) {
  return [
    { value: String(t.portfolios), label: "portfolios" },
    { value: String(t.agentDecisions), label: "agent decisions" },
    { value: formatCompactUsd(t.totalAumUsd), label: "USDC managed" },
    { value: String(t.chains), label: "chains" },
  ];
}

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
      "DeepSeek proposes, GPT mini critiques, Qwen classifies regime — each routed via OpenRouter. Every decision surfaces the model slug it used.",
  },
  {
    icon: Shield,
    title: "Real on-chain execution",
    description:
      "CCTP V2 bridge, Uniswap V3 swaps on Base — route registry validates every leg before approval, never at submit time. USYC and EURC sleeves are coming soon.",
  },
  {
    icon: Zap,
    title: "Transparent reasoning",
    description:
      "Every decision ships with the strategist's prose, the critic's verdict, and a public diary — judge the agent on the receipts, not the demo.",
  },
  {
    icon: Bot,
    title: "You approve every move",
    description:
      "Set your goal once. The agent monitors 24/7 and proposes — you click Approve. Single modal, USDC fee preview, no surprises.",
  },
  {
    icon: TrendingUp,
    title: "Realtime via SSE",
    description:
      "Price ticks, regime flips, agent decisions, and rebalance legs streamed the moment they happen — no polling, no refresh.",
  },
  {
    icon: BarChart3,
    title: "Yield on idle USDC (coming soon)",
    description:
      "USYC via the Hashnote Teller — T-bill rate on idle USDC. The strategist already factors yield into proposals; on-chain execution unlocks once the allowlist opens.",
  },
  {
    icon: FileSpreadsheet,
    title: "Tax-loss harvesting + 1099-DA",
    description:
      "The strategist spots open lots at a loss and proposes a harvest. Export a 1099-DA-ready CSV for your accountant.",
  },
  {
    icon: Trophy,
    title: "Trustability score + peg defense",
    description:
      "A rolling score grades decisions against realized outcomes. Peg-defense auto-proposes rebalances the moment a stablecoin drifts.",
  },
  {
    icon: Sparkles,
    title: "Agent Studio",
    description:
      "Pause, resume, or trigger the agent on demand. See live capital context — idle cash, invested value, gateway status — before every run.",
  },
  {
    icon: Target,
    title: "Agent-managed allocation",
    description:
      "Set your risk tolerance once. The agent targets an allocation that fits your goal — Conservative USDC hold, Balanced, or Growth — and proposes any changes for your approval.",
  },
  {
    icon: Wallet,
    title: "Multi-chain wallets",
    description:
      "Arc + Base wallets in one view. Per-chain USDC balances with live route states — Ready or Coming Soon. USYC and EURC tracked; executable once integrations open.",
  },
  {
    icon: PieChart,
    title: "Portfolio analytics",
    description:
      "Decision quality score, regime overlay, target vs actual allocation drift, and wallet cash breakdown — all in one compact screen.",
  },
];

const CIRCLE_STACK = [
  {
    name: "Circle Wallets",
    sub: "Modular MSCA",
    desc: "Programmable smart account wallets — no seed phrase, sign in with passkey or email.",
  },
  {
    name: "Circle Gateway",
    sub: "Unified balance",
    desc: "Single USDC balance view aggregated across Arc + Base in one call.",
  },
  {
    name: "CCTP V2 + Hooks",
    sub: "Cross-chain",
    desc: "Atomic cross-chain rebalancing with destination hook execution on arrival.",
  },
  {
    name: "USYC",
    sub: "Yield · coming soon",
    desc: "Hashnote T-bill rate on idle USDC — factored into every proposal. On-chain park/redeem is coming soon (Teller allowlist pending).",
  },
  {
    name: "Circle Paymaster",
    sub: "Gas abstraction",
    desc: "Protocol fees settle in USDC. On Arc, gas is paid in USDC too — no native ETH to manage.",
  },
  {
    name: "Arc StableFX",
    sub: "FX rails · coming soon",
    desc: "USDC ↔ EURC conversion at native Arc rates — no CEX, no slippage on stablecoin FX. EURC execution is tracked; live swaps are coming soon.",
  },
  {
    name: "Nanopayments",
    sub: "Fee rails",
    desc: "Per-rebalance fee settlement and referral payouts settled on-chain.",
  },
];

const HOW_IT_WORKS = [
  {
    step: "01",
    title: "Set your goal",
    items: [
      "Risk tolerance (conservative → aggressive)",
      "Target USDC allocation (agent executes today)",
      "USYC / EURC preferences tracked — execution coming soon",
      "Tax-loss harvesting on/off",
    ],
  },
  {
    step: "02",
    title: "Agent proposes",
    items: [
      "Regime classifier reads market state",
      "Strategist (deepseek/deepseek-v4-flash) plans rebalance",
      "Critic (gpt-mini) adversarial review",
      "Proposal queued with full prose reasoning",
    ],
  },
  {
    step: "03",
    title: "You approve",
    items: [
      "Single approval modal — no config maze",
      "USDC fee preview via Nanopayments",
      "USDC-native gas on Arc via Circle Paymaster",
      "Executed on Arc + Base via CCTP V2",
    ],
  },
];

const LABEL_TONE: Record<LeaderboardEntry["label"], string> = {
  excellent: "text-accent-pnl border-accent-pnl/30 bg-accent-pnl/5",
  strong: "text-accent-pnl/80 border-accent-pnl/20 bg-accent-pnl/5",
  stable: "text-text-default border-border-default bg-raised",
  shaky: "text-warn border-amber-500/30 bg-amber-500/5",
  underperforming: "text-risk border-risk/30 bg-risk/5",
};

function signedPct(value: number) {
  const rounded = Math.round(value * 100) / 100;
  const safe = rounded === 0 ? 0 : rounded;
  return `${safe >= 0 ? "+" : ""}${safe.toFixed(1)}%`;
}

export default function LandingPage() {
  const [announcementDismissed, setAnnouncementDismissed] = useState(() => {
    if (typeof window === "undefined") return false;
    return sessionStorage.getItem("aegis.announcement.dismissed") === "1";
  });
  const [mockApproved, setMockApproved] = useState(false);
  const [traction, setTraction] = useState<Traction | null>(null);
  const [topPerformers, setTopPerformers] = useState<LeaderboardEntry[]>([]);
  const [statsLoaded, setStatsLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const [t, board] = await Promise.allSettled([
        tractionApi.get(),
        leaderboardApi.list(LEADERBOARD_PREVIEW_COUNT),
      ]);
      if (cancelled) return;
      if (t.status === "fulfilled") setTraction(t.value);
      if (board.status === "fulfilled") setTopPerformers(board.value);
      setStatsLoaded(true);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const stats = traction ? tractionStats(traction) : null;

  return (
    <div className="min-h-screen bg-bg text-text-hi overflow-hidden">
      {/* Ambient glow — on-system tokens only */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-[-10%] right-[5%] w-[500px] h-[500px] bg-accent-agent/5 rounded-full blur-[120px]" />
        <div className="absolute bottom-[15%] left-[10%] w-[400px] h-[400px] bg-accent-pnl/5 rounded-full blur-[120px]" />
      </div>

      {/* Announcement bar */}
      {!announcementDismissed && (
        <div className="relative z-20 flex items-center justify-center gap-3 px-4 py-2 bg-accent-agent/10 border-b border-accent-agent/20 text-xs font-mono text-accent-agent">
          <Trophy className="w-3.5 h-3.5 shrink-0" />
          <span>
            Built for{" "}
            <span className="font-semibold">Agora Agents Hackathon</span> · RFB
            04 · May 11–25, 2026
          </span>
          <a
            href="https://github.com/ghaaf-labs/aegis"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 underline underline-offset-2 hover:text-accent-agent/80"
          >
            View on GitHub <ExternalLink className="w-3 h-3" />
          </a>
          <button
            type="button"
            onClick={() => {
              sessionStorage.setItem("aegis.announcement.dismissed", "1");
              setAnnouncementDismissed(true);
            }}
            className="absolute right-4 p-1 hover:text-text-hi transition-colors"
            aria-label="Dismiss"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      <LandingHeader />

      <main>
        {/* Hero */}
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
            Set a goal.{" "}
            <span className="text-accent-agent">Agent proposes.</span>
            <br /> You approve.
          </motion.h1>

          <motion.p
            initial="hidden"
            animate="visible"
            variants={fadeUp}
            custom={2}
            className="text-lg text-text-lo max-w-2xl mx-auto mb-6 leading-relaxed"
          >
            Stablecoin-native portfolio management on Arc + Base. Every
            rebalance needs your sign-off — no black box, no surprises, full
            reasoning diary.
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

        {/* Dashboard mockup */}
        <motion.section
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.5, duration: 0.7 }}
          className="relative z-10 max-w-6xl mx-auto px-6 py-20"
        >
          <div className="border-brutal border-border-default bg-surface shadow-brutal">
            {/* Browser chrome */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-border-default bg-raised">
              <div className="flex items-center gap-2">
                <span className="text-xs text-text-mut font-mono">
                  dashboard
                </span>
                <span className="text-[10px] font-mono uppercase tracking-widest text-text-mut border border-border-default px-1.5 py-0.5 rounded-sharp">
                  Illustrative
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="flex items-center gap-1 text-[10px] font-mono text-accent-agent">
                  <span className="w-1.5 h-1.5 rounded-full bg-accent-agent animate-pulse" />
                  arc
                </span>
                <span className="flex items-center gap-1 text-[10px] font-mono text-accent-agent/60">
                  <span className="w-1.5 h-1.5 rounded-full bg-accent-agent/60" />
                  base
                </span>
              </div>
            </div>

            <div className="p-6 grid grid-cols-1 md:grid-cols-[1fr_280px] gap-6">
              {/* Left: portfolio table */}
              <div className="space-y-4">
                <div className="flex items-end justify-between">
                  <div>
                    <p className="text-xs font-mono text-text-lo">
                      Portfolio value
                    </p>
                    <p className="text-3xl font-mono font-bold text-text-hi tabular-nums">
                      $24,180
                      <span className="text-lg">.00</span>
                    </p>
                  </div>
                  <span className="text-sm font-mono text-accent-pnl font-semibold">
                    ↑ +2.4% today
                  </span>
                </div>

                <div className="border-brutal border-border-default overflow-hidden">
                  <table className="w-full text-xs font-mono">
                    <thead className="border-b border-border-default bg-raised text-text-lo">
                      <tr>
                        <th className="text-left px-3 py-2 font-medium">
                          Asset
                        </th>
                        <th className="text-right px-3 py-2 font-medium">
                          Value
                        </th>
                        <th className="text-right px-3 py-2 font-medium">
                          Alloc
                        </th>
                        <th className="text-right px-3 py-2 font-medium">
                          Yield
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {[
                        {
                          asset: "USDC",
                          chain: "Arc",
                          value: "$15,830",
                          alloc: "65%",
                          yield: null,
                        },
                        {
                          asset: "USDC",
                          chain: "Base",
                          value: "$8,350",
                          alloc: "35%",
                          yield: null,
                        },
                      ].map((row) => (
                        <tr
                          key={`${row.asset}-${row.chain}`}
                          className="border-b border-border-default last:border-b-0"
                        >
                          <td className="px-3 py-2.5 text-text-hi font-semibold">
                            {row.asset}
                            <span className="ml-1.5 text-[10px] text-text-mut font-normal">
                              {row.chain}
                            </span>
                          </td>
                          <td className="px-3 py-2.5 text-right tabular-nums text-text-default">
                            {row.value}
                          </td>
                          <td className="px-3 py-2.5 text-right tabular-nums text-text-lo">
                            {row.alloc}
                          </td>
                          <td className="px-3 py-2.5 text-right">
                            {row.yield ? (
                              <span className="text-accent-pnl">
                                {row.yield}
                              </span>
                            ) : (
                              <span className="text-text-mut">—</span>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>

                {/* Approval preview interaction — demo only, no on-chain action */}
                <div
                  className="flex items-center gap-3 p-3 border-brutal border-accent-agent/30 bg-accent-agent/5"
                  role="status"
                  aria-live="polite"
                >
                  {mockApproved ? (
                    <span className="flex items-center gap-2 text-xs font-mono text-text-lo">
                      This is a demo preview — connect a wallet to run real
                      rebalances →{" "}
                      <Link
                        href="/login"
                        className="underline text-accent-agent"
                      >
                        Get started
                      </Link>
                    </span>
                  ) : (
                    <>
                      <span className="text-xs font-mono text-text-lo flex-1">
                        Agent: rebalance USDC across Arc + Base · fee: $0.12
                        USDC
                      </span>
                      <button
                        type="button"
                        onClick={() => setMockApproved(true)}
                        className="shrink-0 px-3 py-1.5 bg-accent-agent text-black text-xs font-mono font-semibold border-brutal border-black rounded-sharp hover:opacity-90 transition-opacity"
                        aria-label="Preview approval (demo only)"
                      >
                        Preview approval
                      </button>
                    </>
                  )}
                </div>
              </div>

              {/* Right: allocation + agent card */}
              <div className="space-y-4">
                <div className="border-brutal border-border-default bg-raised p-4 space-y-3">
                  <p className="text-xs font-mono text-text-lo">Allocation</p>
                  {[
                    { label: "USDC · Arc", pct: 65, color: "bg-accent-agent" },
                    {
                      label: "USDC · Base",
                      pct: 35,
                      color: "bg-accent-agent/50",
                    },
                  ].map((row) => (
                    <div key={row.label} className="space-y-1">
                      <div className="flex justify-between text-[11px] font-mono">
                        <span className="text-text-default">{row.label}</span>
                        <span className="text-text-lo tabular-nums">
                          {row.pct}%
                        </span>
                      </div>
                      <div className="h-1.5 bg-border-default rounded-full overflow-hidden">
                        <div
                          className={cn("h-full rounded-full", row.color)}
                          style={{ width: `${row.pct}%` }}
                        />
                      </div>
                    </div>
                  ))}
                </div>

                <div className="border-brutal border-accent-agent/30 bg-accent-agent/5 p-4 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] font-mono text-accent-agent uppercase tracking-widest">
                      Last decision
                    </span>
                    <span className="text-[10px] font-mono text-text-mut">
                      2m ago
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span className="text-[10px] font-mono px-1.5 py-0.5 border border-accent-agent/30 text-accent-agent rounded-sharp">
                      deepseek/deepseek-v4-flash
                    </span>
                    <div
                      className="flex gap-0.5"
                      role="progressbar"
                      aria-valuenow={82}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuetext="confidence 82%"
                    >
                      {[1, 2, 3, 4].map((d) => (
                        <span
                          key={d}
                          className="w-2 h-2 rounded-full bg-accent-agent"
                        />
                      ))}
                      <span className="w-2 h-2 rounded-full bg-border-default" />
                    </div>
                    <span className="text-[10px] font-mono text-text-mut">
                      82%
                    </span>
                  </div>
                  <p className="text-xs text-text-lo leading-relaxed">
                    &ldquo;Rebalance $800 USDC from Arc to Base — current regime
                    is consolidating, Base liquidity is deeper at this
                    horizon.&rdquo;
                  </p>
                </div>
              </div>
            </div>
          </div>
        </motion.section>

        {/* How it works */}
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

        {/* Agent reasoning showcase */}
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
                  <span className="text-[10px] font-mono text-text-mut">
                    82%
                  </span>
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
                  congestion. No objections; proposal is sound for the
                  consolidating regime.
                </p>
              </div>

              <div className="flex items-center gap-2 text-xs font-mono text-text-mut">
                <span className="w-2 h-2 rounded-full bg-accent-agent" />
                Proposal queued for user approval · USDC fee: $0.12 via
                Nanopayments
              </div>
            </div>
          </motion.div>
        </section>

        {/* Product tour */}
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

        {/* Circle stack */}
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
              Six Circle APIs. Every layer of the product — wallets,
              cross-chain, yield, gas, fees — runs on Circle infrastructure.
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
                <p className="text-xs text-text-lo leading-relaxed">
                  {api.desc}
                </p>
              </motion.div>
            ))}
          </div>
        </section>

        {/* Trust stats bar — live from /api/traction */}
        <div className="relative z-10 border-t border-b border-border-default bg-surface py-6 mb-16">
          <div className="max-w-4xl mx-auto px-6 space-y-4">
            {statsLoaded && !traction && (
              <p className="text-center text-xs font-mono text-text-mut">
                Live usage data is unavailable right now.
              </p>
            )}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-6">
              {(stats ?? STAT_LABELS).map((stat) => (
                <div key={stat.label} className="text-center">
                  <p className="text-3xl font-mono font-bold text-text-hi tabular-nums">
                    {"value" in stat ? (
                      stat.value
                    ) : (
                      <span className="text-text-mut">—</span>
                    )}
                  </p>
                  <p className="text-xs font-mono text-text-mut mt-1">
                    {stat.label}
                  </p>
                </div>
              ))}
            </div>
            {traction && (
              <p className="text-center text-[10px] font-mono text-text-mut">
                Live · via /api/traction
              </p>
            )}
          </div>
        </div>

        {/* Features */}
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

        {/* Leaderboard mini-widget */}
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

        {/* CTA */}
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
              Set your risk tolerance. Connect your portfolio. Every move needs
              your sign-off.
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
      </main>

      <LandingFooter />
    </div>
  );
}
