import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  ShieldCheck,
  XCircle,
} from "lucide-react";
import { BrutalPill } from "@aegis/ui";
import type { WalletLedgerEntry } from "@/lib/api";
import type { RebalanceHistoryRow } from "./shared";

export function KindPill({ kind }: { kind: WalletLedgerKind }) {
  const meta: Record<WalletLedgerKind, { label: string; className: string }> = {
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

export function LedgerStatusPill({ status }: { status: string }) {
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

export function ApprovalStatePill({ row }: { row: RebalanceHistoryRow }) {
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

export function SummaryPill({
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

export function StatusPill({ status }: { status: string }) {
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

type WalletLedgerKind = WalletLedgerEntry["kind"];
