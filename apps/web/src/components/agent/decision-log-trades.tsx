"use client";

import { formatCurrency } from "@/lib/utils";
import type { AgentDecision } from "@/types";
import {
  normalizedTradeAction,
  tradeActionLabel,
  tradeSymbol,
  tradeValueUsd,
  userFacingTradeReason,
  type TradeLike,
} from "./decision-log-utils";

export function TradeTable({
  blocked,
  decisionId,
  trades,
}: {
  blocked: boolean;
  decisionId: string;
  trades: AgentDecision["recommendation"]["trades"];
}) {
  return (
    <div className="mt-3 overflow-hidden border border-border-default">
      <div className="flex items-center justify-between gap-3 border-b border-border-default bg-raised px-3 py-2">
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          Trade legs
        </p>
        <p className="font-mono text-[10px] text-text-mut">
          {trades.length} {trades.length === 1 ? "leg" : "legs"}
        </p>
      </div>
      <div className="grid max-h-64 gap-1.5 overflow-y-auto p-2 sm:hidden">
        {trades.map((trade, ti) => (
          <TradeLegCard
            blocked={blocked}
            decisionId={decisionId}
            index={ti}
            key={`${decisionId}-mobile-${tradeSymbol(trade)}-${ti}`}
            trade={trade}
          />
        ))}
      </div>
      <div className="hidden max-h-64 overflow-y-auto sm:block">
        <table className="w-full table-fixed border-collapse text-[11px]">
          <thead className="sticky top-0 z-10 bg-bg">
            <tr className="border-b border-border-default font-mono text-[10px] uppercase tracking-widest text-text-mut">
              <th className="w-20 px-3 py-2 text-left font-normal">Move</th>
              <th className="w-[22%] px-2 py-2 text-left font-normal">Asset</th>
              <th className="w-[28%] px-2 py-2 text-right font-normal">
                Amount
              </th>
              <th className="px-2 py-2 text-left font-normal">Route</th>
            </tr>
          </thead>
          <tbody>
            {trades.map((trade, ti) => (
              <TradeRow
                blocked={blocked}
                decisionId={decisionId}
                index={ti}
                key={`${decisionId}-${tradeSymbol(trade)}-${ti}`}
                trade={trade}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function TradeLegCard({
  blocked,
  decisionId,
  index,
  trade,
}: {
  blocked: boolean;
  decisionId: string;
  index: number;
  trade: TradeLike;
}) {
  const action = normalizedTradeAction(trade);
  const symbol = tradeSymbol(trade);
  const valueUsd = tradeValueUsd(trade);
  const reason = userFacingTradeReason(trade);
  return (
    <div
      aria-label={`${tradeActionLabel(action)} ${symbol}`}
      className="grid min-h-12 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-border-default bg-bg/70 px-3 py-2"
      data-decision-id={decisionId}
      data-leg-index={index}
    >
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <span
            className={`shrink-0 font-mono text-[10px] ${
              blocked || action === "review"
                ? "text-text-mut"
                : action === "sell"
                  ? "text-risk"
                  : "text-text-lo"
            }`}
          >
            {tradeActionLabel(action)}
          </span>
          <span className="truncate font-mono text-[11px] font-semibold text-text-hi">
            {symbol}
          </span>
        </div>
        <p className="mt-0.5 truncate text-[10px] text-text-mut" title={reason}>
          {reason}
        </p>
      </div>
      <span className="shrink-0 text-right font-mono text-[11px] font-semibold tabular-nums text-accent-pnl">
        {valueUsd != null ? formatCurrency(valueUsd) : "-"}
      </span>
    </div>
  );
}

function TradeRow({
  blocked,
  decisionId,
  index,
  trade,
}: {
  blocked: boolean;
  decisionId: string;
  index: number;
  trade: TradeLike;
}) {
  const action = normalizedTradeAction(trade);
  const symbol = tradeSymbol(trade);
  const valueUsd = tradeValueUsd(trade);
  return (
    <tr
      aria-label={`${tradeActionLabel(action)} ${symbol}`}
      className="border-b border-border-default last:border-b-0"
      data-decision-id={decisionId}
      data-leg-index={index}
    >
      <td
        className={`whitespace-nowrap px-3 py-1.5 font-mono text-[10px] ${
          blocked || action === "review"
            ? "text-text-mut"
            : action === "sell"
              ? "text-risk"
              : "text-text-lo"
        }`}
      >
        {tradeActionLabel(action)}
      </td>
      <td
        className="truncate px-2 py-1.5 font-mono text-text-hi"
        title={symbol}
      >
        {symbol}
      </td>
      <td className="px-2 py-1.5 text-right font-mono tabular-nums text-accent-pnl">
        {valueUsd != null ? formatCurrency(valueUsd) : "-"}
      </td>
      <td
        className="truncate px-2 py-1.5 text-text-mut"
        title={userFacingTradeReason(trade)}
      >
        {userFacingTradeReason(trade)}
      </td>
    </tr>
  );
}
