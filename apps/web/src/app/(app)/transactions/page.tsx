"use client";

import { useEffect, useState } from "react";
import { ListChecks } from "lucide-react";
import { rebalanceApi, walletsApi, type WalletLedgerEntry } from "@/lib/api";
import { useActivePortfolio } from "@/stores/portfolio";
import type { LedgerTab, RebalanceHistoryRow } from "./_components/shared";
import { OnChainLedger } from "./_components/on-chain-ledger";
import { PlanHistory } from "./_components/plan-history";

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
