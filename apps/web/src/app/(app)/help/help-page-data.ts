import {
  History,
  LifeBuoy,
  ShieldCheck,
  type LucideIcon,
  Wallet,
} from "lucide-react";

export type HelpTone = "pnl" | "agent" | "warn" | "risk";

export interface QuickPathItem {
  href: string;
  icon: LucideIcon;
  label: string;
  tone: HelpTone;
  value: string;
}

export interface StatusRowItem {
  action: string;
  meaning: string;
  status: string;
  tone: HelpTone;
}

export const QUICK_PATHS: QuickPathItem[] = [
  {
    href: "/wallets",
    icon: Wallet,
    label: "Wallets",
    value: "cash, routes, addresses",
    tone: "pnl",
  },
  {
    href: "/transactions",
    icon: History,
    label: "Transactions",
    value: "reviews, traces, status",
    tone: "agent",
  },
  {
    href: "/agent-logs",
    icon: ShieldCheck,
    label: "Agent Logs",
    value: "reasoning and critic",
    tone: "agent",
  },
  {
    href: "/policy#refunds",
    icon: LifeBuoy,
    label: "Policy",
    value: "fees and refunds",
    tone: "warn",
  },
];

export const STATUS_ROWS: StatusRowItem[] = [
  {
    status: "Planned",
    meaning: "A review exists, but execution has not started.",
    action: "Open the latest review and approve only if it is still ready.",
    tone: "agent",
  },
  {
    status: "Needs changes",
    meaning: "The route, balance, or capabilities no longer match the review.",
    action: "Build a fresh review from current balances.",
    tone: "warn",
  },
  {
    status: "Failed",
    meaning: "Execution stopped and the row is now audit history.",
    action: "Open trace, read the failed leg, then build a fresh review.",
    tone: "risk",
  },
  {
    status: "Completed",
    meaning:
      "The movement finished and should be reflected in portfolio state.",
    action: "Check Dashboard, Portfolio, and Transactions trace.",
    tone: "pnl",
  },
];

export const SUPPORT_ROWS = [
  [
    "Never paste secrets",
    "Aegis support should not need private keys, seed phrases, or one-time codes.",
  ],
  ["Use IDs", "Share a rebalance ID, decision ID, wallet route, or trace row."],
  [
    "Market losses",
    "Policy can cover platform failures, not price movement after approval.",
  ],
] as const;
