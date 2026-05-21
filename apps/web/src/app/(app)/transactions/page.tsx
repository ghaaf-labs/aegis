"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  ArrowRight,
  CheckCircle2,
  Clock3,
  ListChecks,
  XCircle,
} from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { rebalanceApi } from "@/lib/api";
import { timeAgo } from "@/lib/utils";
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
          confirm whether a plan is still pending, blocked, failed, or settled.
        </p>
      </div>

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
                      <th className="px-3 py-2 font-medium text-right">Legs</th>
                      <th className="px-3 py-2 font-medium">Created</th>
                      <th className="px-3 py-2 font-medium text-right">Open</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => (
                      <tr
                        key={row.id}
                        className="border-b border-white/5 last:border-b-0"
                      >
                        <td className="px-3 py-3 text-text-hi">
                          {row.id.slice(0, 8)}...
                        </td>
                        <td className="px-3 py-3">
                          <StatusPill status={row.status} />
                        </td>
                        <td className="px-3 py-3 text-right tabular-nums text-text-default">
                          {row.completedLegs}/{row.totalLegs}
                        </td>
                        <td className="px-3 py-3 text-text-lo">
                          {timeAgo(row.createdAt)}
                        </td>
                        <td className="px-3 py-3 text-right">
                          <Link
                            href={`/rebalance/${row.id}`}
                            className="inline-flex items-center gap-1 text-accent-agent hover:underline"
                          >
                            Review
                            <ArrowRight className="h-3 w-3" />
                          </Link>
                        </td>
                      </tr>
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
