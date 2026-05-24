"use client";

import type { CSSProperties, ReactElement } from "react";
import { Grid3X3, ShieldAlert } from "lucide-react";
import {
  BrutalCard as Card,
  BrutalCardBody as CardContent,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  ProvenanceLine,
} from "@aegis/ui";
import { TokenBadge } from "@/components/dashboard/token-badge";
import type {
  DashboardBalanceModel,
  DashboardMatrixCell,
} from "@/lib/dashboard-balance-model";
import { cn, formatCurrency, timeAgo } from "@/lib/utils";

interface RouteStackMatrixProps {
  model: DashboardBalanceModel;
}

export function RouteStackMatrix({ model }: RouteStackMatrixProps) {
  return (
    <Card data-testid="route-stack-matrix" className="overflow-hidden">
      <CardHeader className="min-h-[56px]">
        <CardTitle className="flex min-w-0 items-center gap-2">
          <Grid3X3 className="h-3.5 w-3.5 shrink-0 text-accent-agent" />
          <span className="truncate">Route Stack Matrix</span>
        </CardTitle>
        <span className="hidden font-mono text-[10px] text-text-mut md:block">
          Matrix view: tokens vs chains
        </span>
      </CardHeader>

      <CardContent className="p-0 font-mono">
        <CompactRouteRail model={model} />
        <SummaryRail model={model} />
        <ReserveRail model={model} />
        <MatrixTable model={model} />
        <div className="grid gap-2 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
          <p className="text-[10px] leading-relaxed text-text-mut">
            Tokens and chains expand automatically as balances appear. Empty
            routes stay visible while Circle reports them as active wallet
            routes.
          </p>
          <ProvenanceLine
            source="Circle balances + execution ledger"
            freshness={freshness(model)}
            className={model.walletBalanceUnavailable ? "text-warn" : undefined}
          />
        </div>
      </CardContent>
    </Card>
  );
}

function CompactRouteRail({ model }: { model: DashboardBalanceModel }) {
  return (
    <div className="grid grid-cols-2 border-b border-border-default lg:hidden">
      <CompactRouteCell
        label="Wallet routes"
        value={String(model.chainCount)}
        detail={`${model.tokenCount} ${model.tokenCount === 1 ? "token" : "tokens"}`}
      />
      <CompactRouteCell
        label="Route value"
        value={formatCurrency(model.matrixTotalUsd)}
        detail={`${model.reservePct.toFixed(0)}% reserve`}
        tone="pnl"
      />
    </div>
  );
}

function CompactRouteCell({
  detail,
  label,
  tone = "default",
  value,
}: {
  detail: string;
  label: string;
  tone?: "default" | "pnl";
  value: string;
}) {
  return (
    <div className="min-h-16 border-r border-border-default px-4 py-3 last:border-r-0">
      <p className="truncate text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={cn(
          "mt-1 truncate text-base font-semibold tabular-nums",
          tone === "pnl" ? "text-accent-pnl" : "text-text-hi",
        )}
        title={value}
      >
        {value}
      </p>
      <p className="mt-1 truncate text-[10px] text-text-lo" title={detail}>
        {detail}
      </p>
    </div>
  );
}

function SummaryRail({ model }: { model: DashboardBalanceModel }) {
  return (
    <div className="hidden lg:grid lg:grid-cols-4">
      <SummaryCell
        label="Net worth"
        value={formatCurrency(model.netWorthUsd)}
      />
      <SummaryCell
        label="Wallet cash"
        value={formatCurrency(model.walletValueUsd)}
      />
      <SummaryCell label="Invested" value={formatCurrency(model.investedUsd)} />
      <SummaryCell
        label="Status"
        value={model.status.label}
        tone={model.status.tone}
        icon={
          model.status.tone === "warn" ? (
            <ShieldAlert className="h-4 w-4" />
          ) : undefined
        }
      />
    </div>
  );
}

function SummaryCell({
  label,
  value,
  tone = "default",
  icon,
}: {
  label: string;
  value: string;
  tone?: DashboardBalanceModel["status"]["tone"] | "default";
  icon?: ReactElement;
}) {
  return (
    <div className="min-h-[76px] border-b border-r border-border-default px-4 py-3 [&:nth-child(2n)]:border-r-0 lg:[&:nth-child(2n)]:border-r lg:[&:nth-child(4n)]:border-r-0">
      <p className="truncate text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <div className="mt-1 flex min-w-0 items-center gap-2">
        {icon && (
          <span className={cn("h-4 w-4 shrink-0", toneClass(tone))}>
            {icon}
          </span>
        )}
        <p
          className={cn(
            "min-w-0 line-clamp-2 text-base font-semibold tabular-nums",
            toneClass(tone),
          )}
          title={value}
        >
          {value}
        </p>
      </div>
    </div>
  );
}

function ReserveRail({ model }: { model: DashboardBalanceModel }) {
  const deployablePct = model.walletValueUsd
    ? (model.deployableUsd / model.walletValueUsd) * 100
    : 0;

  return (
    <div className="hidden grid-cols-2 border-b border-border-default lg:grid">
      <div className="grid min-h-12 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-r border-border-default px-4 py-2">
        <p className="min-w-0 truncate text-[10px] uppercase tracking-widest text-text-mut">
          Reserve {model.reservePct.toFixed(0)}%
        </p>
        <p className="text-sm font-semibold tabular-nums text-accent-pnl">
          {formatCurrency(model.reserveUsd)}
        </p>
      </div>
      <div className="grid min-h-12 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 px-4 py-2">
        <p className="min-w-0 truncate text-[10px] uppercase tracking-widest text-accent-pnl">
          Deployable {deployablePct.toFixed(0)}%
        </p>
        <p className="text-sm font-semibold tabular-nums text-accent-pnl">
          {formatCurrency(model.deployableUsd)}
        </p>
      </div>
    </div>
  );
}

function MatrixTable({ model }: { model: DashboardBalanceModel }) {
  if (model.matrixRows.length === 0) {
    return (
      <div className="border-b border-border-default px-4 py-5 text-[11px] text-text-lo">
        No route balances yet. Fund a wallet route and the matrix will populate.
      </div>
    );
  }

  return (
    <>
      <MatrixMobileList model={model} />
      <div className="hidden overflow-x-auto border-b border-border-default lg:block">
        <table className="w-full min-w-[760px] border-collapse text-[11px]">
          <thead>
            <tr className="border-b border-border-default bg-bg/45 text-text-mut">
              <th className="sticky left-0 z-10 min-w-[112px] border-r border-border-default bg-bg/95 px-3 py-2 text-left uppercase tracking-widest">
                Token
              </th>
              {model.chains.map((chain) => (
                <th
                  key={chain.key}
                  className="min-w-[112px] border-r border-border-default px-3 py-2 text-right uppercase tracking-widest"
                >
                  {chain.shortLabel}
                </th>
              ))}
              <th className="min-w-[112px] border-r border-border-default px-3 py-2 text-right uppercase tracking-widest">
                Total
              </th>
              <th className="min-w-[68px] px-3 py-2 text-right uppercase tracking-widest">
                %
              </th>
            </tr>
          </thead>
          <tbody>
            {model.matrixRows.map((row) => (
              <tr key={row.symbol} className="border-b border-border-default">
                <th className="sticky left-0 z-10 border-r border-border-default bg-bg/95 px-3 py-2 text-left">
                  <span className="flex min-w-0 items-center gap-2">
                    <TokenBadge
                      symbol={row.symbol}
                      className="h-5 w-5 shrink-0"
                    />
                    <span className="truncate text-text-hi">{row.symbol}</span>
                  </span>
                </th>
                {model.chains.map((chain) => {
                  const cell =
                    row.cells.find((item) => item.chainKey === chain.key) ??
                    emptyCell(chain.key);
                  return (
                    <td
                      key={chain.key}
                      className="border-r border-border-default px-3 py-2 text-right tabular-nums text-text-hi"
                      style={heatStyle(cell.shareOfWalletPct)}
                    >
                      {moneyOrDash(cell.valueUsd)}
                    </td>
                  );
                })}
                <td className="border-r border-border-default px-3 py-2 text-right font-semibold tabular-nums text-text-hi">
                  {formatCurrency(row.totalUsd, { compact: true })}
                </td>
                <td className="px-3 py-2 text-right tabular-nums text-text-lo">
                  {row.weightPct.toFixed(1)}%
                </td>
              </tr>
            ))}
            <tr className="bg-bg/55 font-semibold text-text-hi">
              <th className="sticky left-0 z-10 border-r border-border-default bg-bg/95 px-3 py-2 text-left uppercase tracking-widest">
                Total
              </th>
              {model.chains.map((chain) => {
                const cell =
                  model.matrixTotals.find(
                    (item) => item.chainKey === chain.key,
                  ) ?? emptyCell(chain.key);
                return (
                  <td
                    key={chain.key}
                    className="border-r border-border-default px-3 py-2 text-right tabular-nums"
                  >
                    {moneyOrDash(cell.valueUsd)}
                  </td>
                );
              })}
              <td className="border-r border-border-default px-3 py-2 text-right tabular-nums text-accent-pnl">
                {formatCurrency(model.matrixTotalUsd, { compact: true })}
              </td>
              <td className="px-3 py-2 text-right tabular-nums">100%</td>
            </tr>
          </tbody>
        </table>
      </div>
    </>
  );
}

function MatrixMobileList({ model }: { model: DashboardBalanceModel }) {
  return (
    <div className="border-b border-border-default lg:hidden">
      {model.matrixRows.map((row) => {
        const activeCells = row.cells.filter((cell) => cell.valueUsd > 0.005);
        return (
          <article
            key={row.symbol}
            className="border-b border-border-default px-4 py-3 last:border-b-0"
          >
            <div className="flex min-w-0 items-center justify-between gap-3">
              <span className="flex min-w-0 items-center gap-2">
                <TokenBadge symbol={row.symbol} className="h-5 w-5 shrink-0" />
                <span className="truncate text-sm font-semibold text-text-hi">
                  {row.symbol}
                </span>
              </span>
              <span className="shrink-0 text-right">
                <span className="block text-sm font-semibold tabular-nums text-text-hi">
                  {formatCurrency(row.totalUsd, { compact: true })}
                </span>
                <span className="block text-[10px] tabular-nums text-text-lo">
                  {row.weightPct.toFixed(1)}%
                </span>
              </span>
            </div>
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              {activeCells.length > 0 ? (
                activeCells.map((cell) => {
                  const chain = model.chains.find(
                    (item) => item.key === cell.chainKey,
                  );
                  return (
                    <div
                      key={cell.chainKey}
                      className="grid min-h-10 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-border-default bg-bg/45 px-3 py-2"
                      style={heatStyle(cell.shareOfWalletPct)}
                    >
                      <span className="truncate text-[10px] uppercase tracking-widest text-text-mut">
                        {chain?.shortLabel ?? titleCase(cell.chainKey)}
                      </span>
                      <span className="text-right text-[11px] font-semibold tabular-nums text-text-hi">
                        {moneyOrDash(cell.valueUsd)}
                      </span>
                    </div>
                  );
                })
              ) : (
                <p className="border border-border-default bg-bg/45 px-3 py-2 text-[11px] text-text-lo">
                  No route value yet
                </p>
              )}
            </div>
          </article>
        );
      })}
    </div>
  );
}

function emptyCell(chainKey: string): DashboardMatrixCell {
  return { chainKey, valueUsd: 0, shareOfWalletPct: 0 };
}

function moneyOrDash(value: number) {
  if (value <= 0.005) return "$0.00";
  if (value < 0.01) return "<$0.01";
  return formatCurrency(value, { compact: true });
}

function heatStyle(shareOfWalletPct: number): CSSProperties {
  if (shareOfWalletPct <= 0) return {};
  const opacity = Math.min(0.26, 0.05 + shareOfWalletPct / 220);
  return { backgroundColor: `rgba(0, 255, 136, ${opacity.toFixed(3)})` };
}

function titleCase(value: string) {
  return value
    .replaceAll("-", " ")
    .replaceAll("_", " ")
    .toLowerCase()
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function toneClass(tone: DashboardBalanceModel["status"]["tone"] | "default") {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "warn") return "text-warn";
  if (tone === "risk") return "text-risk";
  if (tone === "muted") return "text-text-lo";
  return "text-text-hi";
}

function freshness(model: DashboardBalanceModel) {
  if (model.walletBalanceUnavailable) return "needs retry";
  if (model.walletBalanceLoading) return "syncing";
  if (!model.gatewayBalanceUpdatedAt) return "live";
  return `refreshed ${timeAgo(new Date(model.gatewayBalanceUpdatedAt).toISOString())}`;
}
