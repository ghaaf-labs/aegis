import type { Metadata } from "next";
import Link from "next/link";
import { BrutalPill } from "@aegis/ui";
import { LandingShell } from "@/components/layout/landing-shell";

export const metadata: Metadata = {
  title: "Aegis · Outcome & Refund Policy",
  description:
    "Plain-English policy: what we refund, what we won't, how a user pauses the agent, and how disputes escalate.",
};

export default function PolicyPage() {
  return (
    <LandingShell>
      <header className="mb-10 pt-4">
        <BrutalPill tone="agent" className="mb-3">
          Operational floor
        </BrutalPill>
        <h1 className="mt-3 text-4xl font-bold text-text-hi tracking-tight">
          Outcome &amp; Refund Policy
        </h1>
        <p className="mt-4 text-sm text-text-lo font-mono leading-relaxed max-w-2xl">
          The one-line version: we refund protocol fees on agent-caused failure,
          never on market losses, and any user can pause the agent in one click.
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
                if you changed your mind after approval but before the first leg
                settled.
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
                  Move funds out before closing
                </span>{" "}
                — approved plans can execute for you from your Aegis account.
                Closing the account requires moving funds out first.
              </span>
            </li>
            <li className="flex gap-3">
              <span className="text-accent-agent shrink-0">→</span>
              <span>
                <span className="text-text-hi font-semibold">
                  Close the account
                </span>{" "}
                — Settings → <em>Close account</em> signs you out and starts
                erasure. PII is anonymized where legally allowed; required tax,
                compliance, and on-chain records may be retained.
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
              "No moving money without your approval modal.",
              "No account closure while funds remain in the wallet.",
            ].map((item) => (
              <li key={item} className="flex gap-3">
                <span className="text-accent-pnl shrink-0">✓</span>
                <span>{item}</span>
              </li>
            ))}
          </ul>
        </section>

        <section
          id="terms"
          className="scroll-mt-24 border-brutal border-border-default bg-raised p-6"
        >
          <h2 className="text-base font-bold text-text-hi uppercase tracking-wider mb-4">
            Terms
          </h2>
          <p className="text-sm font-mono leading-relaxed text-text-lo">
            By continuing, you ask Aegis to verify your email, prepare your
            account for approved portfolio actions, and keep execution paused
            until you approve a plan.
          </p>
        </section>

        <section
          id="privacy"
          className="scroll-mt-24 border-brutal border-border-default bg-raised p-6"
        >
          <h2 className="text-base font-bold text-text-hi uppercase tracking-wider mb-4">
            Privacy Policy
          </h2>
          <p className="text-sm font-mono leading-relaxed text-text-lo">
            Aegis uses your email for sign-in, security notices, export links,
            and account recovery. You can export your data, change your email,
            or close the account from Settings.
          </p>
        </section>
      </div>

      <footer className="mt-10 border-t border-border-default pt-6 text-xs font-mono text-text-mut">
        <p>
          The agent&apos;s hard constraints are published at{" "}
          <Link
            href="/about/constitution"
            className="text-accent-agent hover:underline underline-offset-4"
          >
            /about/constitution
          </Link>
          . Last reviewed 2026-05-16.
        </p>
      </footer>
    </LandingShell>
  );
}
