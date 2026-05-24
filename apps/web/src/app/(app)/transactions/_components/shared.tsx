import Link from "next/link";
import { ArrowRight } from "lucide-react";
import { rebalanceApi, type RebalanceApprovalSafety } from "@/lib/api";

export type RebalanceHistoryRow = Awaited<
  ReturnType<typeof rebalanceApi.history>
>[number];

export type LedgerTab = "onchain" | "plans";

export function MobileFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-border-default bg-surface px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className="mt-1 tabular-nums text-text-hi">{value}</p>
    </div>
  );
}

export function executionModeLabel(mode: string | null | undefined) {
  if (mode === "mock") return "historical test";
  if (mode === "real") return "real execution";
  return "execution review";
}

export function LoadingState() {
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

export function EmptyState({
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

export function rowAction(row: RebalanceHistoryRow): {
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

export function rowMeaning(row: RebalanceHistoryRow) {
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

export function approvalSafetySummary(safety: RebalanceApprovalSafety): string {
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
