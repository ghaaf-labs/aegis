"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  ArrowRight,
  AlertTriangle,
  CheckCircle2,
  Clock3,
  ExternalLink,
  ListChecks,
  Route,
  ShieldCheck,
  XCircle,
} from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import {
  rebalanceApi,
  walletsApi,
  type RebalanceApprovalSafety,
  type WalletLedgerEntry,
} from "@/lib/api";
import { formatCurrency, timeAgo } from "@/lib/utils";
import { walletRouteBadgeLabel } from "@/lib/wallet-routes";
import { useActivePortfolio } from "@/stores/portfolio";

type RebalanceHistoryRow = Awaited<
  ReturnType<typeof rebalanceApi.history>
>[number];

type LedgerTab = "onchain" | "plans";

export default function TransactionsPage() {
  const portfolio = useActivePortfolio();
  const [tab, setTab] = useState<LedgerTab>("onchain");
  const [rows, setRows] = useState<RebalanceHistoryRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ledger, setLedger] = useState<WalletLedgerEntry[]>([]);
  const [ledgerLoading, setLedgerLoading] = useState(false);
  const [ledgerError, setLedgerError] = useState<string | null>(null);

  useEffect(() => {
    if (!portfolio) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    rebalanceApi
      .history(portfolio.id)
      .then((history) => {
        if (!cancelled) setRows(history);
      })
      .catch((e) => {
        if (!cancelled)
          setError(
            e instanceof Error
              ? e.message
              : "Transaction history is unavailable right now.",
          );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [portfolio]);

  // The on-chain ledger is per-wallet (all chains), not per-portfolio, so it
  // loads independently of the active portfolio.
  useEffect(() => {
    let cancelled = false;
    setLedgerLoading(true);
    setLedgerError(null);
    walletsApi
      .transactions()
      .then((entries) => {
        if (!cancelled) setLedger(entries);
      })
      .catch((e) => {
        if (!cancelled)
          setLedgerError(
            e instanceof Error
              ? e.message
              : "On-chain transactions are unavailable right now.",
          );
      })
      .finally(() => {
        if (!cancelled) setLedgerLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          On-chain ledger
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <ListChecks className="h-5 w-5 text-accent-agent" />
          Transactions
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          Every real on-chain move across your wallets — funding, CCTP bridges,
          swaps, approvals — with explorer links. Rebalance-plan history is a
          filter below.
        </p>
      </div>

      <div
        role="tablist"
        aria-label="Transaction view"
        className="flex flex-wrap gap-2"
      >
        <TabButton
          active={tab === "onchain"}
          onClick={() => setTab("onchain")}
          label="On-chain"
          count={ledger.length}
        />
        <TabButton
          active={tab === "plans"}
          onClick={() => setTab("plans")}
          label="Rebalance plans"
          count={rows.length}
        />
      </div>

      {tab === "onchain" ? (
        <OnChainLedger
          entries={ledger}
          loading={ledgerLoading}
          error={ledgerError}
        />
      ) : (
        <PlanHistory
          portfolio={portfolio}
          rows={rows}
          loading={loading}
          error={error}
        />
      )}
    </div>
  );
}

function TabButton({
  active,
  onClick,
  label,
  count,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  count: number;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={`inline-flex min-h-9 items-center gap-2 border px-3 font-mono text-xs ${
        active
          ? "border-accent-agent bg-accent-agent/10 text-accent-agent"
          : "border-border-default bg-bg text-text-lo hover:border-border-hi hover:text-text-hi"
      }`}
    >
      {label}
      <span className="rounded-sharp bg-white/5 px-1.5 py-0.5 text-[10px] tabular-nums text-text-mut">
        {count}
      </span>
    </button>
  );
}

function OnChainLedger({
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

function KindPill({ kind }: { kind: WalletLedgerEntry["kind"] }) {
  const meta: Record<
    WalletLedgerEntry["kind"],
    { label: string; className: string }
  > = {
    deposit: {
      label: "Deposit",
      className: "border-accent-pnl/40 bg-accent-pnl/10 text-accent-pnl",
    },
    bridge: {
      label: "Bridge",
      className: "border-accent-agent/40 bg-accent-agent/10 text-accent-agent",
    },
    swap: {
      label: "Swap",
      className: "border-accent-agent/40 bg-accent-agent/10 text-accent-agent",
    },
    approve: {
      label: "Approve",
      className: "border-border-default bg-bg text-text-lo",
    },
    outbound: {
      label: "Outbound",
      className: "border-warn/40 bg-warn/10 text-warn",
    },
    contract: {
      label: "Contract",
      className: "border-border-default bg-bg text-text-lo",
    },
  };
  const { label, className } = meta[kind] ?? meta.contract;
  return (
    <span
      className={`inline-flex items-center gap-1 border px-1.5 py-0.5 text-[10px] uppercase tracking-widest ${className}`}
    >
      {label}
    </span>
  );
}

function LedgerStatusPill({ status }: { status: string }) {
  const lower = status.toLowerCase();
  if (lower === "complete" || lower === "confirmed" || lower === "completed") {
    return (
      <BrutalPill tone="pnl">
        <CheckCircle2 className="h-3 w-3" />
        Confirmed
      </BrutalPill>
    );
  }
  if (lower === "failed" || lower === "cancelled" || lower === "denied") {
    return (
      <span className="inline-flex items-center gap-1 border border-risk/40 bg-risk/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-risk">
        <XCircle className="h-3 w-3" />
        {status || "failed"}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 border border-warn/40 bg-warn/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-warn">
      <Clock3 className="h-3 w-3" />
      {status || "pending"}
    </span>
  );
}

function PlanHistory({
  portfolio,
  rows,
  loading,
  error,
}: {
  portfolio: ReturnType<typeof useActivePortfolio>;
  rows: RebalanceHistoryRow[];
  loading: boolean;
  error: string | null;
}) {
  return (
    <>
      {rows.length > 0 && (
        <div className="flex flex-wrap gap-2 text-[10px] font-mono">
          <SummaryPill
            label="Completed"
            value={rows.filter((r) => r.status === "completed").length}
            tone="pnl"
          />
          <SummaryPill
            label="Ready"
            value={
              rows.filter(
                (r) =>
                  r.approvalSafety?.approvable === true &&
                  r.executionMode !== "mock",
              ).length
            }
            tone="agent"
          />
          <SummaryPill
            label="Needs changes"
            value={
              rows.filter((r) => r.approvalSafety?.approvable === false).length
            }
            tone="warn"
          />
          <SummaryPill
            label="Failed"
            value={rows.filter((r) => r.status === "failed").length}
            tone="risk"
          />
        </div>
      )}

      <LedgerFlowSvg />

      {!portfolio ? (
        <EmptyState
          title="No portfolio yet"
          body="Create a portfolio target before Aegis can build rebalance-plan history."
          href="/onboarding"
          cta="Create portfolio"
        />
      ) : (
        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">
              {portfolio.name} plans
            </span>
            <span className="text-[11px] font-mono text-text-lo">
              {loading ? "Loading..." : `${rows.length} rows`}
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
            ) : rows.length === 0 ? (
              <EmptyState
                title="No rebalance plans yet"
                body="Build a review from Dashboard or Portfolio. After you approve it, the plan appears here."
                href="/portfolio"
                cta="Review portfolio"
              />
            ) : (
              <>
                <div className="space-y-3 md:hidden">
                  {rows.map((row) => (
                    <HistoryCard key={row.id} row={row} />
                  ))}
                </div>
                <div className="hidden overflow-x-auto md:block">
                  <table className="w-full text-left text-xs font-mono">
                    <thead className="border-b border-border-default text-text-mut">
                      <tr>
                        <th className="px-3 py-2 font-medium">Plan</th>
                        <th className="px-3 py-2 font-medium">Status</th>
                        <th className="px-3 py-2 font-medium">Approval</th>
                        <th className="px-3 py-2 font-medium">Meaning</th>
                        <th className="px-3 py-2 font-medium text-right">
                          Routed
                        </th>
                        <th className="px-3 py-2 font-medium text-right">
                          Legs
                        </th>
                        <th className="px-3 py-2 font-medium">Created</th>
                        <th className="px-3 py-2 font-medium text-right">
                          Open
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map((row) => (
                        <HistoryRow key={row.id} row={row} />
                      ))}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </BrutalCardBody>
        </BrutalCard>
      )}
    </>
  );
}

function HistoryRow({ row }: { row: RebalanceHistoryRow }) {
  const blocked = row.approvalSafety?.approvable === false;
  const next = rowAction(row);
  return (
    <tr className="border-b border-white/5 last:border-b-0 align-top hover:bg-white/[0.02]">
      <td className="px-3 py-3 text-text-hi">
        <div className="flex flex-col gap-1">
          <span>{row.id.slice(0, 8)}...</span>
          <span className="text-[10px] uppercase tracking-widest text-text-mut">
            {executionModeLabel(row.executionMode)}
          </span>
        </div>
      </td>
      <td className="px-3 py-3">
        <StatusPill status={row.status} />
        {row.completedAt && (
          <p className="mt-1 text-[10px] text-text-mut">
            completed {timeAgo(row.completedAt)}
          </p>
        )}
        {row.failureReason && (
          <p className="mt-1 max-w-[220px] text-[10px] leading-relaxed text-risk">
            {row.failureReason}
          </p>
        )}
      </td>
      <td className="px-3 py-3">
        <ApprovalStatePill row={row} />
        {blocked && row.approvalSafety && (
          <div className="mt-1 max-w-[280px] space-y-1 text-[10px] leading-relaxed text-warn">
            <p>{approvalSafetySummary(row.approvalSafety)}</p>
            {row.approvalSafety.missingCapabilities?.length ? (
              <p className="text-text-mut">
                Needed:{" "}
                {row.approvalSafety.missingCapabilities
                  .map((capability) => capability.label)
                  .join(", ")}
              </p>
            ) : null}
          </div>
        )}
      </td>
      <td className="px-3 py-3">
        <p className="max-w-[280px] text-[10px] leading-relaxed text-text-lo">
          {rowMeaning(row)}
        </p>
        <p className="mt-1 text-[10px] uppercase tracking-widest text-text-mut">
          updated {timeAgo(row.updatedAt ?? row.createdAt)}
        </p>
      </td>
      <td className="px-3 py-3 text-right tabular-nums text-text-default">
        {formatCurrency(row.totalAmountUsdc ?? 0)}
        {row.totalGasUsdc != null && row.totalGasUsdc > 0 && (
          <p className="mt-1 text-[10px] text-text-mut">
            gas {formatCurrency(row.totalGasUsdc)}
          </p>
        )}
      </td>
      <td className="px-3 py-3 text-right tabular-nums text-text-default">
        {row.completedLegs}/{row.totalLegs}
      </td>
      <td className="px-3 py-3 text-text-lo">{timeAgo(row.createdAt)}</td>
      <td className="px-3 py-3 text-right">
        <Link
          href={next.href}
          className={`inline-flex min-h-9 items-center gap-1 hover:underline ${
            next.tone === "agent"
              ? "text-accent-agent"
              : next.tone === "pnl"
                ? "text-accent-pnl"
                : "text-warn"
          }`}
        >
          {next.label}
          <ArrowRight className="h-3 w-3" />
        </Link>
      </td>
    </tr>
  );
}

function HistoryCard({ row }: { row: RebalanceHistoryRow }) {
  const blocked = row.approvalSafety?.approvable === false;
  const next = rowAction(row);
  return (
    <article className="border border-border-default bg-bg p-3 font-mono text-xs">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-text-hi">
            {row.id.slice(0, 8)}...
          </p>
          <p className="mt-1 text-[10px] uppercase tracking-widest text-text-mut">
            {executionModeLabel(row.executionMode)}
          </p>
        </div>
        <StatusPill status={row.status} />
      </div>

      <p className="mt-3 text-[11px] leading-relaxed text-text-lo">
        {rowMeaning(row)}
      </p>

      <div className="mt-3 grid grid-cols-2 gap-2">
        <MobileFact
          label="Routed"
          value={formatCurrency(row.totalAmountUsdc ?? 0)}
        />
        <MobileFact
          label="Legs"
          value={`${row.completedLegs}/${row.totalLegs}`}
        />
        <MobileFact label="Created" value={timeAgo(row.createdAt)} />
        <div className="border border-border-default bg-surface px-3 py-2">
          <p className="text-[10px] uppercase tracking-widest text-text-mut">
            Approval
          </p>
          <div className="mt-1">
            <ApprovalStatePill row={row} />
          </div>
        </div>
      </div>

      {blocked && row.approvalSafety && (
        <p className="mt-3 border border-warn/40 bg-warn/5 px-3 py-2 text-[11px] leading-relaxed text-warn">
          {approvalSafetySummary(row.approvalSafety)}
        </p>
      )}

      {row.failureReason && (
        <p className="mt-3 border border-risk/40 bg-risk/5 px-3 py-2 text-[11px] leading-relaxed text-risk">
          {row.failureReason}
        </p>
      )}

      <Link
        href={next.href}
        className={`mt-3 inline-flex min-h-9 w-full items-center justify-center gap-2 border px-3 text-[11px] font-semibold ${
          next.tone === "agent"
            ? "border-accent-agent/40 bg-accent-agent/10 text-accent-agent"
            : next.tone === "pnl"
              ? "border-accent-pnl/40 bg-accent-pnl/10 text-accent-pnl"
              : "border-warn/40 bg-warn/10 text-warn"
        }`}
      >
        {next.label}
        <ArrowRight className="h-3 w-3" />
      </Link>
    </article>
  );
}

function MobileFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-border-default bg-surface px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className="mt-1 tabular-nums text-text-hi">{value}</p>
    </div>
  );
}

function executionModeLabel(mode: string | null | undefined) {
  if (mode === "mock") return "historical test";
  if (mode === "real") return "real execution";
  return "execution review";
}

function LoadingState() {
  return (
    <div
      aria-live="polite"
      className="border border-border-default bg-bg px-4 py-5"
    >
      <p className="text-sm font-mono font-semibold text-text-hi">
        Loading transaction history
      </p>
      <p className="mt-1 max-w-xl text-xs font-mono leading-relaxed text-text-lo">
        Checking approved moves and execution traces for this portfolio.
      </p>
    </div>
  );
}

function ApprovalStatePill({ row }: { row: RebalanceHistoryRow }) {
  const safety = row.approvalSafety;
  if (row.status !== "planned") {
    return (
      <span className="inline-flex items-center gap-1 border border-border-default bg-bg px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-text-mut">
        Trace only
      </span>
    );
  }
  if (row.executionMode === "mock") {
    return (
      <span className="inline-flex items-center gap-1 border border-warn/40 bg-warn/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-warn">
        <AlertTriangle className="h-3 w-3" />
        Audit only
      </span>
    );
  }
  if (safety?.approvable) {
    return (
      <span className="inline-flex items-center gap-1 border border-accent-agent/40 bg-accent-agent/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-accent-agent">
        <ShieldCheck className="h-3 w-3" />
        Ready
      </span>
    );
  }
  if (safety?.approvable === false) {
    return (
      <span className="inline-flex items-center gap-1 border border-warn/40 bg-warn/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-warn">
        <AlertTriangle className="h-3 w-3" />
        {safety.code === "SUPERSEDED" ? "Superseded" : "Needs changes"}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 border border-border-default bg-bg px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-text-mut">
      Unknown
    </span>
  );
}

function SummaryPill({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone: "agent" | "pnl" | "warn" | "risk";
}) {
  const className =
    tone === "agent"
      ? "border-accent-agent/40 bg-accent-agent/10 text-accent-agent"
      : tone === "pnl"
        ? "border-accent-pnl/40 bg-accent-pnl/10 text-accent-pnl"
        : tone === "warn"
          ? "border-warn/40 bg-warn/10 text-warn"
          : "border-risk/40 bg-risk/10 text-risk";
  return (
    <span
      className={`inline-flex items-center gap-1 border px-2 py-1 uppercase tracking-widest ${className}`}
    >
      {label}: {value}
    </span>
  );
}

function rowAction(row: RebalanceHistoryRow): {
  href: string;
  label: string;
  tone: "agent" | "pnl" | "warn";
} {
  if (row.status === "completed") {
    return { href: `/rebalance/${row.id}`, label: "View trace", tone: "pnl" };
  }
  if (row.status === "failed" || row.status === "executing") {
    return { href: `/rebalance/${row.id}`, label: "Open trace", tone: "warn" };
  }
  if (row.approvalSafety?.approvable) {
    return { href: `/rebalance/${row.id}`, label: "Review", tone: "agent" };
  }
  if (row.status === "planned" && row.approvalSafety?.approvable === false) {
    return { href: `/rebalance/${row.id}`, label: "Open review", tone: "warn" };
  }
  return {
    href: `/dashboard/${row.portfolioId}`,
    label: "Fresh review",
    tone: "warn",
  };
}

function rowMeaning(row: RebalanceHistoryRow) {
  const status = row.status.toLowerCase();
  if (status === "completed") {
    return "Execution finished. The trace shows which legs confirmed and the dashboard should now reflect resulting positions or wallet cash.";
  }
  if (status === "executing") {
    return "Execution has started. Open the trace to see submitted legs, confirmations, and chain-specific waits.";
  }
  if (status === "failed") {
    return "Execution stopped before all legs confirmed. Open the trace, read the failure reason, then build a fresh review.";
  }
  if (row.executionMode === "mock") {
    return "Historical test review, shown for audit only. It cannot be approved for real execution — build a fresh review before money moves.";
  }
  if (row.approvalSafety?.approvable) {
    return "This is the latest review and can still be approved. Nothing moves until the approval screen is confirmed.";
  }
  if (row.approvalSafety?.code === "SUPERSEDED") {
    return "A newer review exists. This row is audit history and should not be approved.";
  }
  if (row.approvalSafety?.code === "STALE_PLAN") {
    return "Wallet cash or holdings changed after this review was built. Rebuild before approving.";
  }
  if (row.approvalSafety?.code === "BALANCE_UNAVAILABLE") {
    return "Wallet cash could not be verified. Open the review for the leg audit, then check Wallets before rebuilding.";
  }
  if (row.approvalSafety?.code === "EXECUTION_UNAVAILABLE") {
    return "One selected route is not ready to move money. Open the review, change the target mix, then rebuild.";
  }
  return row.approvalSafety?.message ?? "This row is kept for audit history.";
}

function approvalSafetySummary(safety: RebalanceApprovalSafety): string {
  switch (safety.code) {
    case "EXECUTION_UNAVAILABLE":
      return "One selected route is not ready to move money. Change the target mix, then build a fresh executable review.";
    case "SUPERSEDED":
      return "A newer review exists for this portfolio.";
    case "STALE_PLAN":
      return "Wallet cash or holdings changed after this review was built.";
    case "BALANCE_UNAVAILABLE":
      return "Wallet cash cannot be verified right now.";
    case "MOCK_OR_LEGACY_PLAN":
      return "This review was created outside the current real-execution path.";
    default:
      return safety.message || "Approval needs changes for this review.";
  }
}

function LedgerFlowSvg() {
  return (
    <div className="border-brutal border-border-default bg-surface p-4 shadow-brutal-sm">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
            Money movement
          </p>
          <p className="mt-1 font-mono text-xs text-text-lo">
            A review is only a proposal. A transaction is created after you
            approve it.
          </p>
        </div>
        <Route className="h-4 w-4 shrink-0 text-accent-agent" />
      </div>
      <svg
        viewBox="0 0 760 180"
        role="img"
        aria-label="Transaction flow from review to approval to completed history"
        className="h-auto w-full border border-border-default bg-bg"
      >
        <defs>
          <pattern
            id="ledger-grid"
            width="22"
            height="22"
            patternUnits="userSpaceOnUse"
          >
            <path d="M22 0H0V22" fill="none" stroke="#242424" strokeWidth="1" />
          </pattern>
          <filter id="ledger-glow" x="-25%" y="-25%" width="150%" height="150%">
            <feGaussianBlur stdDeviation="2.5" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <rect width="760" height="180" fill="url(#ledger-grid)" />
        <path
          d="M126 90H278H432H586"
          fill="none"
          stroke="#67e8f9"
          strokeDasharray="9 7"
          strokeWidth="4"
          filter="url(#ledger-glow)"
        >
          <animate
            attributeName="stroke-dashoffset"
            dur="2.4s"
            from="32"
            repeatCount="indefinite"
            to="0"
          />
        </path>
        <LedgerNode x={58} title="Review" subtitle="proposal" tone="agent" />
        <LedgerNode
          x={254}
          title="Approve"
          subtitle="your choice"
          tone="agent"
        />
        <LedgerNode
          x={450}
          title="Move funds"
          subtitle="after approval"
          tone="pnl"
        />
        <LedgerNode x={614} title="History" subtitle="result" tone="neutral" />
      </svg>
    </div>
  );
}

function LedgerNode({
  x,
  title,
  subtitle,
  tone,
}: {
  x: number;
  title: string;
  subtitle: string;
  tone: "agent" | "pnl" | "neutral";
}) {
  const stroke =
    tone === "agent" ? "#67e8f9" : tone === "pnl" ? "#86efac" : "#737373";
  const fill =
    tone === "agent" ? "#082f49" : tone === "pnl" ? "#052e16" : "#111111";
  return (
    <g>
      <rect
        x={x}
        y="52"
        width="104"
        height="76"
        fill={fill}
        stroke={stroke}
        strokeWidth="3"
      />
      <rect x={x + 13} y="66" width="78" height="10" fill={stroke} />
      <text
        x={x + 52}
        y="101"
        fill="#f5f5f5"
        fontFamily="monospace"
        fontSize="13"
        fontWeight="700"
        textAnchor="middle"
      >
        {title}
      </text>
      <text
        x={x + 52}
        y="117"
        fill="#a3a3a3"
        fontFamily="monospace"
        fontSize="9"
        textAnchor="middle"
      >
        {subtitle}
      </text>
    </g>
  );
}

function StatusPill({ status }: { status: string }) {
  const lower = status.toLowerCase();
  if (lower === "completed") {
    return (
      <BrutalPill tone="pnl">
        <CheckCircle2 className="h-3 w-3" />
        Completed
      </BrutalPill>
    );
  }
  if (lower === "failed" || lower === "blocked") {
    return (
      <span className="inline-flex items-center gap-1 border border-risk/40 bg-risk/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-risk">
        <XCircle className="h-3 w-3" />
        {status}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 border border-warn/40 bg-warn/10 px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-warn">
      <Clock3 className="h-3 w-3" />
      {status}
    </span>
  );
}

function EmptyState({
  title,
  body,
  href,
  cta,
}: {
  title: string;
  body: string;
  href: string;
  cta: string;
}) {
  return (
    <div className="border border-border-default bg-bg px-4 py-5">
      <p className="text-sm font-mono font-semibold text-text-hi">{title}</p>
      <p className="mt-1 max-w-xl text-xs font-mono leading-relaxed text-text-lo">
        {body}
      </p>
      <Link
        href={href}
        className="mt-3 inline-flex min-h-9 items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent hover:border-accent-agent"
      >
        {cta}
        <ArrowRight className="h-3 w-3" />
      </Link>
    </div>
  );
}
