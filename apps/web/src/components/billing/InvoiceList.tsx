"use client";

import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
  type PillTone,
} from "@aegis/ui";
import type { Invoice, InvoiceStatus } from "@/types";
import { explorerTxUrl } from "@/lib/explorers";

const STATUS_TONE: Record<InvoiceStatus, PillTone> = {
  paid: "pnl",
  open: "neutral",
  past_due: "risk",
  pastDue: "risk",
  void: "neutral",
};

function formatPeriod(start: string, end: string): string {
  const s = new Date(start);
  const e = new Date(end);
  const fmt = (d: Date) =>
    d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  return `${fmt(s)} – ${fmt(e)}, ${e.getUTCFullYear()}`;
}

export interface InvoiceListProps {
  invoices: Invoice[];
  /** Optional title; defaults to "Invoices". */
  title?: string;
}

export function InvoiceList({
  invoices,
  title = "Invoices",
}: InvoiceListProps) {
  if (invoices.length === 0) {
    return (
      <BrutalCard data-testid="invoice-list-empty">
        <BrutalCardHeader>
          <span className="text-sm font-semibold text-text-hi">{title}</span>
          <BrutalPill tone="neutral">FREE</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody>
          <p className="text-sm text-text-default">
            No invoices yet — you&apos;re on the Free plan.
          </p>
          <p className="text-[11px] font-mono text-text-lo mt-2">
            Upgrades and AUM-fee accruals will appear here after they settle.
          </p>
        </BrutalCardBody>
      </BrutalCard>
    );
  }

  return (
    <BrutalCard data-testid="invoice-list">
      <BrutalCardHeader>
        <span className="text-sm font-semibold text-text-hi">{title}</span>
        <span className="text-[11px] font-mono text-text-lo">
          {invoices.length} total
        </span>
      </BrutalCardHeader>
      <div className="overflow-x-auto">
        <table className="w-full text-xs font-mono">
          <thead className="text-text-lo border-b border-border-default">
            <tr>
              <th className="text-left px-4 py-2 font-medium">Period</th>
              <th className="text-left px-4 py-2 font-medium">Items</th>
              <th className="text-right px-4 py-2 font-medium">Subtotal</th>
              <th className="text-right px-4 py-2 font-medium">Total</th>
              <th className="text-left px-4 py-2 font-medium">Status</th>
              <th className="text-left px-4 py-2 font-medium">Tx</th>
            </tr>
          </thead>
          <tbody>
            {invoices.map((inv) => (
              <tr
                key={inv.id}
                className="border-b border-white/5 last:border-b-0"
                data-invoice-id={inv.id}
              >
                <td className="px-4 py-2 text-text-default">
                  {formatPeriod(inv.periodStart, inv.periodEnd)}
                </td>
                <td className="px-4 py-2 text-text-default">
                  {inv.lineItems.length}
                </td>
                <td className="px-4 py-2 text-right tabular-nums text-text-default">
                  ${inv.subtotalUsdc.toFixed(2)}
                </td>
                <td className="px-4 py-2 text-right tabular-nums text-text-hi">
                  ${inv.totalUsdc.toFixed(2)}
                </td>
                <td className="px-4 py-2">
                  <BrutalPill tone={STATUS_TONE[inv.status]}>
                    {inv.status.replace("_", " ").toUpperCase()}
                  </BrutalPill>
                </td>
                <td className="px-4 py-2">
                  {inv.paidTxHash ? (
                    <a
                      href={explorerTxUrl("arc", inv.paidTxHash) ?? "#"}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-accent-pnl hover:text-accent-pnl/60 underline-offset-2 hover:underline"
                      title={inv.paidTxHash}
                    >
                      {inv.paidTxHash.slice(0, 6)}…{inv.paidTxHash.slice(-4)}
                    </a>
                  ) : (
                    <span className="text-text-lo">—</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </BrutalCard>
  );
}
