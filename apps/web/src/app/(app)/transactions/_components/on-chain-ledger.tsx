import { ExternalLink } from "lucide-react";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";
import type { WalletLedgerEntry } from "@/lib/api";
import { timeAgo } from "@/lib/utils";
import { walletRouteBadgeLabel } from "@/lib/wallet-routes";
import { EmptyState, LoadingState, MobileFact } from "./shared";
import { KindPill, LedgerStatusPill } from "./pills";

export function OnChainLedger({
  entries,
  loading,
  error,
}: {
  entries: WalletLedgerEntry[];
  loading: boolean;
  error: string | null;
}) {
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-sm font-mono text-text-hi">
          All-wallet activity
        </span>
        <span className="text-[11px] font-mono text-text-lo">
          {loading ? "Loading..." : `${entries.length} transactions`}
        </span>
      </BrutalCardHeader>
      <BrutalCardBody>
        {error && (
          <p
            aria-live="polite"
            className="mb-3 border border-risk/40 bg-risk/5 px-3 py-2 text-xs font-mono text-risk"
          >
            {error}
          </p>
        )}
        {loading ? (
          <LoadingState />
        ) : entries.length === 0 ? (
          <EmptyState
            title="No on-chain transactions yet"
            body="Fund a wallet or approve a plan. Deposits, bridges, swaps, and approvals across every chain appear here with explorer links."
            href="/wallets"
            cta="Open wallets"
          />
        ) : (
          <>
            <div className="space-y-3 md:hidden">
              {entries.map((entry) => (
                <LedgerCard key={entry.id} entry={entry} />
              ))}
            </div>
            <div className="hidden overflow-x-auto md:block">
              <table className="w-full text-left text-xs font-mono">
                <thead className="border-b border-border-default text-text-mut">
                  <tr>
                    <th className="px-3 py-2 font-medium">Type</th>
                    <th className="px-3 py-2 font-medium">Chain</th>
                    <th className="px-3 py-2 font-medium">Token</th>
                    <th className="px-3 py-2 font-medium text-right">Amount</th>
                    <th className="px-3 py-2 font-medium">Status</th>
                    <th className="px-3 py-2 font-medium">When</th>
                    <th className="px-3 py-2 font-medium text-right">
                      Explorer
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((entry) => (
                    <LedgerRow key={entry.id} entry={entry} />
                  ))}
                </tbody>
              </table>
            </div>
          </>
        )}
      </BrutalCardBody>
    </BrutalCard>
  );
}

function LedgerRow({ entry }: { entry: WalletLedgerEntry }) {
  return (
    <tr className="border-b border-white/5 align-top last:border-b-0 hover:bg-white/[0.02]">
      <td className="px-3 py-3">
        <KindPill kind={entry.kind} />
      </td>
      <td className="px-3 py-3 text-text-hi">
        {walletRouteBadgeLabel(entry.chain)}
      </td>
      <td className="px-3 py-3 text-text-default">{entry.token ?? "—"}</td>
      <td className="px-3 py-3 text-right tabular-nums text-text-default">
        {entry.amount ?? "—"}
      </td>
      <td className="px-3 py-3">
        <LedgerStatusPill status={entry.status} />
      </td>
      <td className="px-3 py-3 text-text-lo">
        {entry.date ? timeAgo(entry.date) : "—"}
      </td>
      <td className="px-3 py-3 text-right">
        {entry.explorerUrl ? (
          <a
            href={entry.explorerUrl}
            target="_blank"
            rel="noreferrer"
            className="inline-flex min-h-8 items-center gap-1 text-accent-agent hover:underline"
          >
            View
            <ExternalLink className="h-3 w-3" />
          </a>
        ) : (
          <span className="text-text-mut">—</span>
        )}
      </td>
    </tr>
  );
}

function LedgerCard({ entry }: { entry: WalletLedgerEntry }) {
  return (
    <article className="border border-border-default bg-bg p-3 font-mono text-xs">
      <div className="flex items-start justify-between gap-3">
        <KindPill kind={entry.kind} />
        <LedgerStatusPill status={entry.status} />
      </div>
      <div className="mt-3 grid grid-cols-2 gap-2">
        <MobileFact label="Chain" value={walletRouteBadgeLabel(entry.chain)} />
        <MobileFact label="Token" value={entry.token ?? "—"} />
        <MobileFact label="Amount" value={entry.amount ?? "—"} />
        <MobileFact
          label="When"
          value={entry.date ? timeAgo(entry.date) : "—"}
        />
      </div>
      {entry.explorerUrl && (
        <a
          href={entry.explorerUrl}
          target="_blank"
          rel="noreferrer"
          className="mt-3 inline-flex min-h-9 w-full items-center justify-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 text-[11px] font-semibold text-accent-agent"
        >
          View on explorer
          <ExternalLink className="h-3 w-3" />
        </a>
      )}
    </article>
  );
}
