import Link from "next/link";
import {
  ArrowRight,
  CircleHelp,
  LifeBuoy,
  ReceiptText,
  ShieldAlert,
  Wallet,
} from "lucide-react";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";

const HELP_ITEMS = [
  {
    href: "/wallets",
    icon: Wallet,
    title: "Why does wallet cash show $0?",
    body: "Wallets shows idle USDC/EURC only. Invested positions live on Dashboard and Portfolio.",
  },
  {
    href: "/transactions",
    icon: ShieldAlert,
    title: "Why is approval blocked?",
    body: "Transactions keeps stale, mock, failed, and completed plans visible without letting old plans execute.",
  },
  {
    href: "/agent-logs",
    icon: LifeBuoy,
    title: "What did the agent decide?",
    body: "Agent Logs shows the model slug, confidence, critic verdict, and recommendation summary.",
  },
  {
    href: "/tax-center",
    icon: ReceiptText,
    title: "How do tax exports work?",
    body: "Tax center exports settled transaction rows and signed accountant links with clear caveats.",
  },
];

export default function HelpPage() {
  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Product guide
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <CircleHelp className="h-5 w-5 text-accent-agent" />
          Help
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          Quick links for the confusing parts of Aegis: wallet cash, approvals,
          agent reasoning, tax exports, and support policy.
        </p>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        {HELP_ITEMS.map((item) => (
          <Link key={item.href} href={item.href} className="group">
            <BrutalCard className="h-full group-hover:border-accent-agent/50">
              <BrutalCardHeader>
                <div className="flex items-center gap-2">
                  <item.icon className="h-4 w-4 text-accent-agent" />
                  <span className="text-sm font-mono text-text-hi">
                    {item.title}
                  </span>
                </div>
                <ArrowRight className="h-4 w-4 text-text-mut group-hover:text-accent-agent" />
              </BrutalCardHeader>
              <BrutalCardBody>
                <p className="text-xs font-mono leading-relaxed text-text-lo">
                  {item.body}
                </p>
              </BrutalCardBody>
            </BrutalCard>
          </Link>
        ))}
      </div>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">Support policy</span>
        </BrutalCardHeader>
        <BrutalCardBody>
          <p className="text-sm leading-relaxed text-text-lo">
            Aegis refunds protocol fees for agent-caused execution failures,
            never market losses. The full policy page explains pause controls,
            dispute handling, and refund boundaries.
          </p>
          <Link
            href="/policy"
            className="mt-3 inline-flex items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent hover:border-accent-agent"
          >
            Open policy
            <ArrowRight className="h-3 w-3" />
          </Link>
        </BrutalCardBody>
      </BrutalCard>
    </div>
  );
}
