import type { Metadata } from "next";
import Link from "next/link";
import { Shield } from "lucide-react";
import { BrutalPill } from "@aegis/ui";

export const metadata: Metadata = {
  title: "Aegis · Outcome & Refund Policy",
  description:
    "Plain-English policy: what we refund, what we won't, how a user pauses the agent, and how disputes escalate.",
};

export default function PolicyPage() {
  return (
    <div className="min-h-screen bg-bg text-text-default">
      {/* Gradient orbs */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-[-20%] left-[10%] w-[600px] h-[600px] bg-blue-600/10 rounded-full blur-[120px]" />
        <div className="absolute bottom-[10%] right-[5%] w-[400px] h-[400px] bg-cyan-600/10 rounded-full blur-[100px]" />
      </div>

      {/* Nav */}
      <nav className="relative z-10 flex items-center justify-between px-6 py-5 max-w-4xl mx-auto">
        <Link href="/" className="flex items-center gap-2 group">
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-bold text-lg tracking-tight text-text-hi group-hover:text-accent-agent transition-colors">
            Aegis
          </span>
        </Link>
        <Link
          href="/"
          className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
        >
          ← Back to home
        </Link>
      </nav>

      {/* Content */}
      <main className="relative z-10 mx-auto max-w-4xl px-6 pb-20">
        <header className="mb-10 pt-4">
          <BrutalPill tone="agent" className="mb-3">
            Operational floor
          </BrutalPill>
          <h1 className="mt-3 text-4xl font-bold text-text-hi tracking-tight">
            Outcome &amp; Refund Policy
          </h1>
          <p className="mt-4 text-sm text-text-lo font-mono leading-relaxed max-w-2xl">
            The one-line version: we refund protocol fees on agent-caused
            failure, never on market losses, and any user can pause the agent in
            one click.
          </p>
        </header>

        <div className="space-y-6">
          {/* What we refund */}
          <section className="border-brutal border-border-default bg-raised p-6">
            <h2 className="text-base font-bold text-text-hi uppercase tracking-wider mb-4">
              What we refund
            </h2>
            <ul className="space-y-3 text-sm font-mono">
              <li className="flex gap-3">
                <span className="text-accent-pnl font-bold shrink-0">✓</span>
                <span className="text-text-lo">
                  <span className="text-accent-pnl font-semibold">
                    Full fee refund
                  </span>{" "}
                  if a rebalance failed mid-execution (CCTP attestation timeout,
                  RPC outage, on-chain revert) — no service was delivered.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-pnl font-bold shrink-0">✓</span>
                <span className="text-text-lo">
                  <span className="text-accent-pnl font-semibold">
                    Full fee refund
                  </span>{" "}
                  if the agent&apos;s recommendation violated a constitution
                  clause, plus a written explanation of the violation.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-pnl font-bold shrink-0">~</span>
                <span className="text-text-lo">
                  <span className="text-accent-pnl font-semibold">
                    Pro-rata refund
                  </span>{" "}
                  if part of the plan landed and part couldn&apos;t complete due
                  to liquidity.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-pnl font-bold shrink-0">✓</span>
                <span className="text-text-lo">
                  <span className="text-accent-pnl font-semibold">
                    Full refund
                  </span>{" "}
                  if you changed your mind after approval but before the first
                  leg settled.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-risk font-bold shrink-0">✗</span>
                <span className="text-text-lo">
                  <span className="text-risk font-semibold">No refund</span> for
                  market losses on a successfully executed plan. Market risk is
                  yours.
                </span>
              </li>
            </ul>
          </section>

          {/* How you take back control */}
          <section className="border-brutal border-border-default bg-raised p-6">
            <h2 className="text-base font-bold text-text-hi uppercase tracking-wider mb-4">
              How you take back control
            </h2>
            <ul className="space-y-3 text-sm font-mono text-text-lo">
              <li className="flex gap-3">
                <span className="text-accent-agent shrink-0">→</span>
                <span>
                  <span className="text-text-hi font-semibold">
                    Pause the agent
                  </span>{" "}
                  — one toggle in{" "}
                  <Link
                    href="/settings"
                    className="text-accent-agent hover:underline underline-offset-4"
                  >
                    Settings
                  </Link>{" "}
                  stops every scheduled trigger immediately.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-agent shrink-0">→</span>
                <span>
                  <span className="text-text-hi font-semibold">
                    Cancel pending legs
                  </span>{" "}
                  — pre-approval is a no-op. After approval, in-flight legs
                  settle; pending ones aren&apos;t retried.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-agent shrink-0">→</span>
                <span>
                  <span className="text-text-hi font-semibold">
                    Withdraw at any time
                  </span>{" "}
                  — Aegis is non-custodial; your USDC sits in your own Circle
                  Wallet.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-agent shrink-0">→</span>
                <span>
                  <span className="text-text-hi font-semibold">
                    Delete the account
                  </span>{" "}
                  — Settings → <em>Delete account</em> drops portfolio +
                  decisions + PII. On-chain history stays on-chain.
                </span>
              </li>
            </ul>
          </section>

          {/* Dispute escalation */}
          <section className="border-brutal border-border-default bg-raised p-6">
            <h2 className="text-base font-bold text-text-hi uppercase tracking-wider mb-4">
              Dispute escalation
            </h2>
            <ol className="space-y-3 text-sm font-mono text-text-lo">
              <li className="flex gap-3">
                <span className="text-accent-agent font-bold shrink-0 w-4">
                  1.
                </span>
                <span>
                  Read the public decision trace at{" "}
                  <code className="text-accent-agent bg-accent-agent/10 px-1 py-0.5">
                    /decision/&lt;id&gt;
                  </code>{" "}
                  — every decision is open-by-default.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-agent font-bold shrink-0 w-4">
                  2.
                </span>
                <span>
                  Email{" "}
                  <code className="text-accent-agent bg-accent-agent/10 px-1 py-0.5">
                    support@aegis.local
                  </code>{" "}
                  with the rebalance UUID. Reply within 1 business day.
                </span>
              </li>
              <li className="flex gap-3">
                <span className="text-accent-agent font-bold shrink-0 w-4">
                  3.
                </span>
                <span>
                  Unresolved cases escalate to manual operator review within 5
                  business days, with refund or written denial.
                </span>
              </li>
            </ol>
          </section>

          {/* What we won't do */}
          <section className="border-brutal border-border-default bg-raised p-6">
            <h2 className="text-base font-bold text-text-hi uppercase tracking-wider mb-4">
              What we won&apos;t do
            </h2>
            <ul className="space-y-2 text-sm font-mono text-text-lo">
              {[
                "No hidden swap spreads.",
                "No charging on failed execution.",
                "No mandatory KYC at signup.",
                "No moving money without your approval modal.",
              ].map((item) => (
                <li key={item} className="flex gap-3">
                  <span className="text-accent-pnl shrink-0">✓</span>
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </section>
        </div>

        <footer className="mt-10 border-t border-border-default pt-6 text-xs font-mono text-text-mut">
          <p>
            Full operational policy and constitution clauses live in the repo at{" "}
            <code className="text-text-lo">
              docs/11-agent-outcome-policy.md
            </code>{" "}
            and{" "}
            <Link
              href="/about/constitution"
              className="text-accent-agent hover:underline underline-offset-4"
            >
              /about/constitution
            </Link>
            . Last reviewed 2026-05-16.
          </p>
        </footer>
      </main>
    </div>
  );
}
