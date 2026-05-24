import {
  BarChart3,
  Bot,
  Brain,
  FileSpreadsheet,
  PieChart,
  Shield,
  Sparkles,
  Target,
  TrendingUp,
  Trophy,
  Wallet,
  Zap,
  type LucideIcon,
} from "lucide-react";
import type { LeaderboardEntry, Traction } from "@/lib/api";

export const LEADERBOARD_PREVIEW_COUNT = 3;

/** Compact USD formatting for the trust-stats bar: $0 → "$0", 1_240 →
 *  "$1.2k", 142_000 → "$142k", 3_200_000 → "$3.2M". */
export function formatCompactUsd(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "$0";
  if (value < 1_000) return `$${Math.round(value)}`;
  if (value < 1_000_000) {
    const k = value / 1_000;
    return `$${k < 10 ? k.toFixed(1).replace(/\.0$/, "") : Math.round(k)}k`;
  }
  const m = value / 1_000_000;
  return `$${m < 10 ? m.toFixed(1).replace(/\.0$/, "") : Math.round(m)}M`;
}

export const STAT_LABELS = [
  { label: "portfolios" },
  { label: "agent decisions" },
  { label: "USDC managed" },
  { label: "chains" },
] as const;

export function tractionStats(t: Traction) {
  return [
    { value: String(t.portfolios), label: "portfolios" },
    { value: String(t.agentDecisions), label: "agent decisions" },
    { value: formatCompactUsd(t.totalAumUsd), label: "USDC managed" },
    { value: String(t.chains), label: "chains" },
  ];
}

export const fadeUp = {
  hidden: { opacity: 0, y: 24 },
  visible: (i = 0) => ({
    opacity: 1,
    y: 0,
    transition: { delay: i * 0.1, duration: 0.5, ease: "easeOut" },
  }),
};

export const FEATURES: Array<{
  icon: LucideIcon;
  title: string;
  description: string;
}> = [
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

export const CIRCLE_STACK = [
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

export const HOW_IT_WORKS = [
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

export const LABEL_TONE: Record<LeaderboardEntry["label"], string> = {
  excellent: "text-accent-pnl border-accent-pnl/30 bg-accent-pnl/5",
  strong: "text-accent-pnl/80 border-accent-pnl/20 bg-accent-pnl/5",
  stable: "text-text-default border-border-default bg-raised",
  shaky: "text-warn border-amber-500/30 bg-amber-500/5",
  underperforming: "text-risk border-risk/30 bg-risk/5",
};

export function signedPct(value: number) {
  const rounded = Math.round(value * 100) / 100;
  const safe = rounded === 0 ? 0 : rounded;
  return `${safe >= 0 ? "+" : ""}${safe.toFixed(1)}%`;
}
