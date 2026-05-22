"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  ArrowRight,
  AlertTriangle,
  CheckCircle2,
  Clock3,
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
import { rebalanceApi } from "@/lib/api";
import { formatCurrency, timeAgo } from "@/lib/utils";
import { useActivePortfolio } from "@/stores/portfolio";

type RebalanceHistoryRow = Awaited<
  ReturnType<typeof rebalanceApi.history>
>[number];

export default function TransactionsPage() {
  const portfolio = useActivePortfolio();
  const [rows, setRows] = useState<RebalanceHistoryRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
          setError(e instanceof Error ? e.message : "Failed to load history");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [portfolio]);

  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Execution ledger
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <ListChecks className="h-5 w-5 text-accent-agent" />
          Transactions
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          Every rebalance review becomes a ledger row here. Use this page to
          distinguish a ready review from stale, blocked, failed, or settled
          execution history.
        </p>
        {rows.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-2 text-[10px] font-mono">
            <SummaryPill
              label="Completed"
              value={rows.filter((r) => r.status === "completed").length}
              tone="pnl"
            />
            <SummaryPill
              label="Ready"
              value={
                rows.filter((r) => r.approvalSafety?.approvable === true).length
              }
              tone="agent"
            />
            <SummaryPill
              label="Blocked"
              value={
                rows.filter((r) => r.approvalSafety?.approvable === false)
                  .length
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
      </div>

      <LedgerFlowSvg />

      {!portfolio ? (
        <EmptyState
          title="No portfolio yet"
          body="Create a portfolio target before Aegis can build transaction history."
          href="/onboarding"
          cta="Create portfolio"
        />
      ) : (
        <BrutalCard>
          <BrutalCardHeader>
            <span className="text-sm font-mono text-text-hi">
              {portfolio.name} activity
            </span>
            <span className="text-[11px] font-mono text-text-lo">
              {loading ? "Loading..." : `${rows.length} rows`}
            </span>
          </BrutalCardHeader>
          <BrutalCardBody>
            {error && (
              <p className="mb-3 border border-risk/40 bg-risk/5 px-3 py-2 text-xs font-mono text-risk">
                {error}
              </p>
            )}
            {rows.length === 0 ? (
              <EmptyState
                title={
                  loading
                    ? "Loading transaction history"
                    : "No transactions yet"
                }
                body="Run Review rebalance from Dashboard or Portfolio. Approved plans and blocked stale reviews will appear here."
                href="/portfolio"
                cta="Review portfolio"
              />
            ) : (
              <div className="overflow-x-auto">
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
                      <th className="px-3 py-2 font-medium text-right">Legs</th>
                      <th className="px-3 py-2 font-medium">Created</th>
                      <th className="px-3 py-2 font-medium text-right">Open</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => (
                      <HistoryRow key={row.id} row={row} />
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </BrutalCardBody>
        </BrutalCard>
      )}
    </div>
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
            {row.executionMode ?? "real"} mode
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
        {blocked && row.approvalSafety?.message && (
          <div className="mt-1 max-w-[280px] space-y-1 text-[10px] leading-relaxed text-warn">
            <p>{row.approvalSafety.message}</p>
            {row.approvalSafety.missingCapabilities?.length ? (
              <p className="text-text-mut">
                Missing:{" "}
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
          className={`inline-flex items-center gap-1 hover:underline ${
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

function ApprovalStatePill({ row }: { row: RebalanceHistoryRow }) {
  const safety = row.approvalSafety;
  if (row.status !== "planned") {
    return (
      <span className="inline-flex items-center gap-1 border border-border-default bg-bg px-1.5 py-0.5 text-[10px] uppercase tracking-widest text-text-mut">
        Trace only
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
        {safety.code === "SUPERSEDED" ? "Superseded" : "Blocked"}
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
    return { href: `/rebalance/${row.id}`, label: "Open block", tone: "warn" };
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
    return "Gateway balance could not be verified. Open the blocked review for the leg audit, then check Wallets before rebuilding.";
  }
  if (row.approvalSafety?.code === "EXECUTION_UNAVAILABLE") {
    return "The review has executable legs, but this API build lacks one or more real adapters. Open the block details before changing targets.";
  }
  return row.approvalSafety?.message ?? "This row is kept for audit history.";
}

function LedgerFlowSvg() {
  return (
    <div className="border-brutal border-border-default bg-surface p-4 shadow-brutal-sm">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
            Ledger path
          </p>
          <p className="mt-1 font-mono text-xs text-text-lo">
            Review rows are not transactions until approval starts execution.
          </p>
        </div>
        <Route className="h-4 w-4 shrink-0 text-accent-agent" />
      </div>
      <svg
        viewBox="0 0 760 180"
        role="img"
        aria-label="Transaction ledger flow from review to approval to execution trace"
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
        <LedgerNode x={58} title="Review" subtitle="no movement" tone="agent" />
        <LedgerNode x={254} title="Approve" subtitle="user gate" tone="agent" />
        <LedgerNode x={450} title="Execute" subtitle="legs + gas" tone="pnl" />
        <LedgerNode x={614} title="Trace" subtitle="audit row" tone="neutral" />
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
        className="mt-3 inline-flex items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent hover:border-accent-agent"
      >
        {cta}
        <ArrowRight className="h-3 w-3" />
      </Link>
    </div>
  );
}
