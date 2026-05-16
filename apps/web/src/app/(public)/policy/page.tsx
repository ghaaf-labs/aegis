import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Aegis · Outcome & Refund Policy",
  description:
    "Plain-English policy: what we refund, what we won't, how a user pauses the agent, and how disputes escalate.",
};

export default function PolicyPage() {
  return (
    <main className="mx-auto max-w-3xl px-6 py-16 text-gray-200">
      <header className="mb-10">
        <p className="text-xs uppercase tracking-widest text-cyan-400">
          Operational floor
        </p>
        <h1 className="mt-2 text-4xl font-semibold text-white">
          Outcome &amp; Refund Policy
        </h1>
        <p className="mt-4 text-sm text-gray-400">
          The one-line version: we refund protocol fees on agent-caused failure,
          never on market losses, and any user can pause the agent in one click.
        </p>
      </header>

      <section className="space-y-6">
        <div>
          <h2 className="text-xl font-semibold text-white">What we refund</h2>
          <ul className="mt-3 space-y-2 text-sm">
            <li>
              <strong className="text-emerald-400">Full fee refund</strong> if a
              rebalance failed mid-execution (CCTP attestation timeout, RPC
              outage, on-chain revert) — no service was delivered.
            </li>
            <li>
              <strong className="text-emerald-400">Full fee refund</strong> if
              the agent&apos;s recommendation violated a constitution clause,
              plus a written explanation of the violation.
            </li>
            <li>
              <strong className="text-emerald-400">Pro-rata refund</strong> if
              part of the plan landed and part couldn&apos;t complete due to
              liquidity.
            </li>
            <li>
              <strong className="text-emerald-400">Full refund</strong> if you
              changed your mind after approval but before the first leg settled.
            </li>
            <li>
              <strong className="text-rose-400">No refund</strong> for market
              losses on a successfully executed plan. Market risk is yours.
            </li>
          </ul>
        </div>

        <div>
          <h2 className="text-xl font-semibold text-white">
            How you take back control
          </h2>
          <ul className="mt-3 space-y-2 text-sm">
            <li>
              <strong>Pause the agent</strong> — one toggle in{" "}
              <Link
                href="/settings"
                className="text-cyan-400 underline-offset-4 hover:underline"
              >
                Settings
              </Link>{" "}
              stops every scheduled trigger immediately.
            </li>
            <li>
              <strong>Cancel pending legs</strong> — pre-approval is a no-op.
              After approval, in-flight legs settle; pending ones aren&apos;t
              retried.
            </li>
            <li>
              <strong>Withdraw at any time</strong> — Aegis is non-custodial;
              your USDC sits in your own Circle Wallet.
            </li>
            <li>
              <strong>Delete the account</strong> — Settings →{" "}
              <em>Delete account</em> drops portfolio + decisions + PII.
              On-chain history stays on-chain.
            </li>
          </ul>
        </div>

        <div>
          <h2 className="text-xl font-semibold text-white">
            Dispute escalation
          </h2>
          <ol className="mt-3 list-decimal space-y-2 pl-6 text-sm">
            <li>
              Read the public decision trace at{" "}
              <code className="text-cyan-300">/decision/&lt;id&gt;</code> —
              every decision is open-by-default.
            </li>
            <li>
              Email <code className="text-cyan-300">support@aegis.local</code>{" "}
              with the rebalance UUID. Reply within 1 business day.
            </li>
            <li>
              Unresolved cases escalate to manual operator review within 5
              business days, with refund or written denial.
            </li>
          </ol>
        </div>

        <div>
          <h2 className="text-xl font-semibold text-white">
            What we won&apos;t do
          </h2>
          <ul className="mt-3 space-y-2 text-sm text-gray-300">
            <li>No hidden swap spreads.</li>
            <li>No charging on failed execution.</li>
            <li>No mandatory KYC at signup.</li>
            <li>No moving money without your approval modal.</li>
          </ul>
        </div>
      </section>

      <footer className="mt-12 border-t border-white/10 pt-6 text-xs text-gray-500">
        <p>
          The full operational policy and constitution clauses live in the repo
          at <code>docs/11-agent-outcome-policy.md</code> and{" "}
          <Link
            href="/about/constitution"
            className="text-cyan-400 underline-offset-4 hover:underline"
          >
            /about/constitution
          </Link>
          . Last reviewed 2026-05-16.
        </p>
      </footer>
    </main>
  );
}
