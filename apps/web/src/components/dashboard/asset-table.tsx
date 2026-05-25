"use client";

import {
  ArrowRight,
  CircleAlert,
  ClipboardCheck,
  Loader2,
  PieChart,
  TrendingUp,
  TrendingDown,
  Wallet,
} from "lucide-react";
import {
  BrutalCard as Card,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  BrutalCardBody as CardContent,
  BrutalBadge as Badge,
} from "@aegis/ui";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import {
  formatCurrency,
  formatPercent,
  formatNumber,
  changeColor,
} from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { targetAllocationsForPortfolio } from "@/components/dashboard/target-allocations";
import type { DashboardBalanceModel } from "@/lib/dashboard-balance-model";

interface AssetTableProps {
  model?: DashboardBalanceModel;
  onReviewPlan?: () => void;
  reviewPlanDisabled?: boolean;
  reviewPlanLoading?: boolean;
}

type TargetAllocationRow = ReturnType<
  typeof targetAllocationsForPortfolio
>[number];

type HoldingRow = {
  alloc: TargetAllocationRow;
  currentWeight: number;
  displayPriceUsd: number | null | undefined;
  drift: number;
  driftAbs: number;
  investedUsd: number;
  price: { change24h: number } | undefined;
  valueUsd: number;
  walletUsd: number;
};

export function AssetTable({
  model,
  onReviewPlan,
  reviewPlanDisabled = false,
  reviewPlanLoading = false,
}: AssetTableProps = {}) {
  const portfolio = useActivePortfolio();
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const livePrices = usePortfolioStore((s) => s.livePrices);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  // Live-tick source is the truthful provenance — FallbackProvider switches
  // between defillama and pyth dynamically, hardcoding either name lies
  // whenever the breaker is open or a single primary call failed.
  const liveSource = Object.values(livePrices)[0]?.source;

  if (!portfolio) return null;

  const ageMs = snapshot
    ? Date.now() - new Date(snapshot.capturedAt).getTime()
    : 0;
  const isStale = ageMs > 60_000;
  const isVeryStale = ageMs > 300_000;
  const priceColor = isVeryStale
    ? "text-risk"
    : isStale
      ? "text-warn"
      : "text-text-hi";

  // USYC is a tokenized USD instrument from Hashnote — not on DefiLlama's
  // free feed, so the snapshot omits it. Fall back to $1.00 (the floor;
  // accrued yield is small relative to the dashboard scale) so the row
  // doesn't read as "broken price feed".
  const snapshotAssets = snapshot?.assets ?? [];
  const priceMap = Object.fromEntries(
    snapshotAssets.map((a) => [a.symbol, a]),
  ) as Record<string, (typeof snapshotAssets)[number] | undefined>;
  if (!priceMap.USDC) {
    priceMap.USDC = stableAsset("USDC", 1, snapshot?.capturedAt);
  }
  if (!priceMap.EURC) {
    priceMap.EURC = stableAsset(
      "EURC",
      model?.eurcUsd ?? 1,
      snapshot?.capturedAt,
    );
  }
  if (!priceMap.USYC) {
    priceMap.USYC = stableAsset("USYC", 1, snapshot?.capturedAt);
  }
  const walletCashKnown = gatewayBalanceStatus === "ready";
  const walletCashUnavailable = gatewayBalanceStatus === "error";
  const walletCashUsd = walletCashKnown
    ? (model?.unifiedUsdc ?? unifiedUsdc)
    : 0;
  const walletEurc = model?.unifiedEurc ?? unifiedEurc;

  const allocList = targetAllocationsForPortfolio(portfolio);
  const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const valueBySymbol = Object.fromEntries(
    metrics.positions.map((position) => [position.symbol, position]),
  );
  const modelHoldingRows =
    model?.tokens
      .filter((token) => token.totalUsd > 0.005)
      .map((token) => {
        const price = priceMap[token.symbol];
        const position = valueBySymbol[token.symbol];
        const displayPriceUsd =
          price?.priceUsd ??
          (position && position.quantity > 0
            ? position.valueUsd / position.quantity
            : null);
        const quantity = displayQuantity({
          symbol: token.symbol,
          valueUsd: token.totalUsd,
          displayedPriceUsd: displayPriceUsd,
          storedQuantity: position?.quantity ?? 0,
        });
        return {
          alloc: {
            assetId: `holding-${token.symbol}`,
            symbol: token.symbol,
            quantity,
            targetWeight: token.targetWeight,
            currentWeight: token.weightPct,
            valueUsd: token.totalUsd,
          },
          currentWeight: token.weightPct,
          displayPriceUsd,
          drift: token.weightPct - token.targetWeight,
          driftAbs: Math.abs(token.weightPct - token.targetWeight),
          investedUsd: token.investedUsd,
          price,
          valueUsd: token.totalUsd,
          walletUsd: token.walletUsd,
        };
      }) ?? [];
  const isUninvested = (model?.investedUsd ?? metrics.investedUsd) < 0.5;
  const hasWalletCash = isUninvested && walletCashUsd > 0.5;
  const hasUsdcSleeve = allocList.some(
    (a) => a.symbol === "USDC" && a.targetWeight > 0,
  );
  const hasTargets = allocList.length > 0;
  const tableTitle =
    model && !isUninvested
      ? "Current Exposure"
      : hasWalletCash
        ? "After Approval"
        : isUninvested
          ? hasTargets
            ? "Target Details"
            : "Portfolio Details"
          : "Current Holdings";
  const holdingRows =
    modelHoldingRows.length > 0
      ? modelHoldingRows
      : allocList.map((alloc) => {
          const price = priceMap[alloc.symbol];
          const position = valueBySymbol[alloc.symbol];
          const currentWeight = position?.currentWeight ?? 0;
          const liveValueUsd = (price?.priceUsd ?? 0) * alloc.quantity;
          // USDC is held as wallet cash, not a confirmed position, so its
          // `alloc.valueUsd`/quantity are 0 — show the live wallet-cash
          // balance instead of $0.00.
          const valueUsd =
            alloc.symbol === "USDC"
              ? walletCashUsd
              : (position?.valueUsd ??
                (liveValueUsd > 0 ? liveValueUsd : alloc.valueUsd));
          const fallbackPriceUsd =
            alloc.quantity > 0 && alloc.valueUsd > 0
              ? alloc.valueUsd / alloc.quantity
              : null;
          const displayPriceUsd = price?.priceUsd ?? fallbackPriceUsd;
          const drift = currentWeight - alloc.targetWeight;
          const driftAbs = Math.abs(drift);
          return {
            alloc,
            currentWeight,
            displayPriceUsd,
            drift,
            driftAbs,
            investedUsd: alloc.symbol === "USDC" ? 0 : valueUsd,
            price,
            valueUsd,
            walletUsd: alloc.symbol === "USDC" ? valueUsd : 0,
          };
        });

  return (
    <Card className="flex h-full min-h-[360px] flex-col">
      <CardHeader className="min-h-[52px] shrink-0">
        <CardTitle className="flex items-center gap-2">
          <PieChart className="h-3.5 w-3.5 text-accent-agent" />
          {tableTitle}
        </CardTitle>
      </CardHeader>
      {hasWalletCash && (
        <div className="mx-4 mb-4 mt-4 rounded-sharp border-brutal border-accent-pnl/40 bg-accent-pnl/5 p-3 sm:mx-5">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-start gap-2">
              <Wallet className="mt-0.5 h-4 w-4 shrink-0 text-accent-pnl" />
              <div>
                <p className="text-sm font-mono font-semibold text-text-hi">
                  {formatCurrency(walletCashUsd)} USDC stays in your wallet
                  until you approve.
                </p>
                <p className="mt-1 text-xs font-mono text-text-lo leading-relaxed">
                  This preview shows the target mix before anything moves.
                  {walletEurc > 0 &&
                    " Existing EURC cash stays in Wallet until you approve a move for it."}
                </p>
              </div>
            </div>
            <button
              type="button"
              onClick={onReviewPlan}
              disabled={!onReviewPlan || reviewPlanDisabled}
              className="inline-flex min-h-10 w-full shrink-0 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-pnl px-3 py-2 text-xs font-mono font-semibold text-black shadow-brutal-sm transition-[box-shadow] hover:shadow-brutal disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none sm:min-h-11 sm:w-auto"
            >
              {reviewPlanLoading ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  Preparing…
                </>
              ) : (
                <>
                  Review plan
                  <ArrowRight className="h-3.5 w-3.5" />
                </>
              )}
            </button>
          </div>
        </div>
      )}
      {walletCashUnavailable && (
        <div className="mx-5 mb-4 mt-4 border-brutal border-warn/50 bg-warn/5 p-3 rounded-sharp">
          <div className="flex items-start gap-2">
            <CircleAlert className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
            <div>
              <p className="text-sm font-mono font-semibold text-text-hi">
                Wallet cash is unknown
              </p>
              <p className="mt-1 text-xs font-mono text-text-lo leading-relaxed">
                {gatewayBalanceError ??
                  "Aegis could not confirm the current wallet balance."}{" "}
                This table shows only the saved target or current holdings until
                the balance check succeeds.
              </p>
            </div>
          </div>
        </div>
      )}
      <CardContent className="flex flex-1 flex-col p-0">
        {!hasTargets && !hasWalletCash ? (
          <div className="px-5 py-10 text-center">
            <div className="mx-auto flex h-9 w-9 items-center justify-center rounded-sharp border border-accent-agent/30 bg-accent-agent/5 text-accent-agent">
              <PieChart className="h-4 w-4" aria-hidden="true" />
            </div>
            <p className="mt-3 font-mono text-sm font-semibold text-text-hi">
              No target mix yet
            </p>
            <p className="mx-auto mt-1 max-w-md font-mono text-xs leading-relaxed text-text-lo">
              The agent has not saved a portfolio target for this account yet.
              Add test USDC or re-open onboarding to generate the first
              proposal.
            </p>
          </div>
        ) : hasWalletCash ? (
          <>
            <TargetPreviewList
              allocList={allocList}
              walletCashUsd={walletCashUsd}
            />
            <div className="hidden auto-rows-fr gap-2 px-5 pb-5 sm:grid sm:grid-cols-2 xl:grid-cols-3">
              {allocList.map((alloc) => (
                <TargetPreviewCard
                  key={alloc.symbol}
                  alloc={alloc}
                  plannedUsd={plannedValueUsd(alloc, walletCashUsd)}
                />
              ))}
            </div>
          </>
        ) : (
          <>
            <div className="grid gap-2 px-5 pb-5 lg:hidden">
              {holdingRows.map((row) => (
                <AssetMobileRow
                  key={row.alloc.symbol}
                  row={row}
                  isUninvested={isUninvested}
                  priceColor={priceColor}
                />
              ))}
            </div>
            <table className="hidden w-full table-fixed lg:table">
              <thead>
                <tr className="border-b border-white/5">
                  {(
                    [
                      ["Asset", ""],
                      ["Price", ""],
                      ["24h", "hidden xl:table-cell"],
                      ["Units", "hidden xl:table-cell"],
                      ["Value", ""],
                      ["Weight vs Target", "hidden xl:table-cell"],
                    ] as const
                  ).map(([h, cls]) => (
                    <th
                      key={h}
                      className={
                        "px-3 py-3 text-left font-mono text-[11px] font-medium uppercase tracking-wider text-text-mut md:px-4 " +
                        cls
                      }
                    >
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {holdingRows.map((row, i) => (
                  <AssetTableRow
                    key={row.alloc.symbol}
                    row={row}
                    isLast={i === holdingRows.length - 1}
                    isUninvested={isUninvested}
                    priceColor={priceColor}
                  />
                ))}
              </tbody>
            </table>
          </>
        )}
        <div className="mt-auto border-t border-white/5 px-5 py-2 font-mono text-[10px] text-text-mut">
          {model && !walletCashUnavailable
            ? "Values reconcile Circle balances with the execution ledger; units derive from live prices when wallet quantities are unavailable"
            : hasWalletCash
              ? hasUsdcSleeve
                ? "USDC target stays as reserve cash; other targets wait for approval"
                : "Targets wait for approval before funds move"
              : !hasTargets
                ? "No target allocation is saved for this portfolio yet"
                : walletCashUnavailable
                  ? "Wallet cash unavailable; planned values are hidden until the balance check succeeds"
                  : !walletCashKnown
                    ? "Waiting for wallet cash before calculating planned values"
                    : isUninvested
                      ? "Targets are configured, but no confirmed position value exists yet"
                      : snapshot
                        ? `Prices via ${liveSource ?? "live feed"} · live snapshot`
                        : "Values use last confirmed holdings while live prices warm up"}
        </div>
      </CardContent>
    </Card>
  );
}

function stableAsset(
  symbol: string,
  priceUsd: number,
  capturedAt: string | undefined,
) {
  return {
    symbol,
    priceUsd,
    change24h: 0,
    change7d: 0,
    marketCap: 0,
    volume24h: 0,
    updatedAt: capturedAt ?? new Date().toISOString(),
  };
}

function displayQuantity({
  displayedPriceUsd,
  storedQuantity,
  symbol,
  valueUsd,
}: {
  displayedPriceUsd: number | null | undefined;
  storedQuantity: number;
  symbol: string;
  valueUsd: number;
}) {
  if (symbol === "USDC" || symbol === "USYC") return valueUsd;
  if (symbol === "EURC") {
    return displayedPriceUsd && displayedPriceUsd > 0
      ? valueUsd / displayedPriceUsd
      : valueUsd;
  }
  if (!displayedPriceUsd || displayedPriceUsd <= 0) return storedQuantity;
  const impliedQuantity = valueUsd / displayedPriceUsd;
  if (storedQuantity <= 0) return impliedQuantity;
  const storedValueUsd = storedQuantity * displayedPriceUsd;
  const ratio = storedValueUsd / valueUsd;
  return ratio >= 0.4 && ratio <= 2.5 ? storedQuantity : impliedQuantity;
}

function TargetPreviewList({
  allocList,
  walletCashUsd,
}: {
  allocList: TargetAllocationRow[];
  walletCashUsd: number;
}) {
  return (
    <div className="px-4 pb-4 sm:hidden">
      <div className="overflow-hidden rounded-sharp border border-border-default bg-surface font-mono">
        {allocList.map((alloc, index) => {
          const isUsdcReserve = alloc.symbol === "USDC";
          return (
            <div
              key={alloc.symbol}
              className={`grid min-h-[58px] grid-cols-[minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5 ${
                index === allocList.length - 1
                  ? ""
                  : "border-b border-border-default"
              }`}
            >
              <div className="min-w-0">
                <div className="flex min-w-0 items-center gap-2">
                  <span className="inline-flex h-6 min-w-10 items-center justify-center rounded-sharp border border-accent-agent/35 bg-accent-agent/10 px-2 text-[10px] font-semibold text-text-hi">
                    {alloc.symbol}
                  </span>
                  <span className="truncate text-sm font-semibold text-text-hi">
                    {isUsdcReserve ? "Cash reserve" : `${alloc.symbol} target`}
                  </span>
                </div>
                <p className="mt-1 truncate text-[10px] uppercase tracking-wider text-text-mut">
                  {alloc.targetWeight.toFixed(0)}% target ·{" "}
                  {isUsdcReserve ? "reserve cash" : "after approval"}
                </p>
              </div>
              <p className="shrink-0 text-right text-sm font-semibold tabular-nums text-text-hi">
                {formatCurrency(plannedValueUsd(alloc, walletCashUsd))}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function TargetPreviewCard({
  alloc,
  plannedUsd,
}: {
  alloc: TargetAllocationRow;
  plannedUsd: number;
}) {
  const isUsdcReserve = alloc.symbol === "USDC";
  return (
    <div className="flex h-full min-h-[128px] flex-col justify-between rounded-sharp border border-border-default bg-surface p-3 font-mono">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="inline-flex h-7 min-w-10 items-center justify-center rounded-sharp border border-accent-agent/35 bg-accent-agent/10 px-2 text-[10px] font-semibold text-text-hi">
            {alloc.symbol}
          </span>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold text-text-hi">
              {isUsdcReserve ? "Cash reserve" : `${alloc.symbol} target`}
            </p>
            <p className="text-[10px] uppercase tracking-wider text-text-mut">
              {alloc.targetWeight.toFixed(0)}% target
            </p>
          </div>
        </div>
        <ClipboardCheck className="h-4 w-4 shrink-0 text-accent-agent/70" />
      </div>
      <div className="mt-3">
        <p className="text-xl font-semibold tabular-nums text-text-hi">
          {formatCurrency(plannedUsd)}
        </p>
        <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
          {isUsdcReserve ? "Held as reserve cash." : "Ready after approval."}
        </p>
      </div>
    </div>
  );
}

function plannedValueUsd(
  alloc: TargetAllocationRow,
  walletCashUsd: number,
): number {
  return walletCashUsd * (alloc.targetWeight / 100);
}

function AssetMobileRow({
  row,
  isUninvested,
  priceColor,
}: {
  row: HoldingRow;
  isUninvested: boolean;
  priceColor: string;
}) {
  const { alloc, price } = row;
  return (
    <div className="rounded-sharp border border-border-default bg-surface p-3 font-mono">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <div className="flex h-7 min-w-10 items-center justify-center rounded-sharp border border-accent-agent/30 bg-accent-agent/10 px-1.5">
            <span className="text-[9px] font-bold text-text-hi">
              {alloc.symbol}
            </span>
          </div>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold text-text-hi">
              {alloc.symbol}
            </p>
            <p className="text-[10px] uppercase tracking-wider text-text-mut">
              {exposureSource(row)} · target {alloc.targetWeight.toFixed(0)}%
            </p>
          </div>
        </div>
        <p className="shrink-0 text-right text-sm font-semibold tabular-nums text-text-hi">
          {formatCurrency(row.valueUsd)}
        </p>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-2 text-[11px]">
        <AssetMetric
          label="Price"
          value={
            row.displayPriceUsd ? formatCurrency(row.displayPriceUsd) : "—"
          }
          className={priceColor}
        />
        <AssetMetric
          label="24h"
          value={price ? formatPercent(price.change24h) : "—"}
          className={price ? changeColor(price.change24h) : "text-text-mut"}
        />
        <AssetMetric
          label="Units"
          value={isUninvested ? "none" : formatNumber(alloc.quantity)}
        />
        <div className="min-h-12 border border-border-default bg-bg/70 px-2 py-1.5">
          <p className="text-[10px] uppercase tracking-wider text-text-mut">
            Weight
          </p>
          {isUninvested ? (
            <p className="mt-0.5 text-text-lo">
              target {alloc.targetWeight.toFixed(0)}%
            </p>
          ) : (
            <div className="mt-0.5 flex flex-wrap items-center gap-1">
              <span className="tabular-nums text-text-lo">
                {row.currentWeight.toFixed(1)}%
              </span>
              <span className="text-text-mut">vs</span>
              <span className="tabular-nums text-text-mut">
                {alloc.targetWeight.toFixed(0)}%
              </span>
              {row.driftAbs > 3 && (
                <Badge
                  variant={row.driftAbs > 10 ? "danger" : "warning"}
                  className="px-1.5 py-0 text-[10px]"
                >
                  {row.drift > 0 ? "+" : ""}
                  {row.drift.toFixed(1)}%
                </Badge>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function AssetMetric({
  label,
  value,
  className = "text-text-hi",
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="min-h-12 min-w-0 border border-border-default bg-bg/70 px-2 py-1.5">
      <p className="text-[10px] uppercase tracking-wider text-text-mut">
        {label}
      </p>
      <p className={`mt-0.5 truncate tabular-nums ${className}`}>{value}</p>
    </div>
  );
}

function AssetTableRow({
  row,
  isLast,
  isUninvested,
  priceColor,
}: {
  row: HoldingRow;
  isLast: boolean;
  isUninvested: boolean;
  priceColor: string;
}) {
  const { alloc, price } = row;
  return (
    <tr
      className={`border-b border-white/3 hover:bg-white/2 transition-colors ${
        isLast ? "border-0" : ""
      }`}
    >
      <td className="px-3 py-3.5 md:px-4">
        <div className="flex items-center gap-2.5">
          <div className="flex h-7 min-w-10 items-center justify-center rounded-sharp border border-accent-agent/30 bg-accent-agent/10 px-1.5">
            <span className="text-[9px] font-bold text-text-hi">
              {alloc.symbol}
            </span>
          </div>
          <div className="min-w-0">
            <span className="block truncate text-sm font-semibold text-text-hi font-mono">
              {alloc.symbol}
            </span>
            <span className="block truncate font-mono text-[10px] uppercase tracking-wider text-text-mut">
              {exposureSource(row)}
            </span>
          </div>
        </div>
      </td>
      <td
        className={`px-3 py-3.5 font-mono text-sm font-medium tabular-nums md:px-4 ${priceColor}`}
      >
        {row.displayPriceUsd ? formatCurrency(row.displayPriceUsd) : "—"}
      </td>
      <td className="hidden px-3 py-3.5 md:px-4 xl:table-cell">
        {price && (
          <span
            className={`text-xs flex items-center gap-1 ${changeColor(price.change24h)}`}
          >
            {price.change24h >= 0 ? (
              <TrendingUp className="w-3 h-3" />
            ) : (
              <TrendingDown className="w-3 h-3" />
            )}
            {formatPercent(price.change24h)}
          </span>
        )}
      </td>
      <td className="hidden px-3 py-3.5 text-xs font-mono text-text-lo md:px-4 xl:table-cell">
        {isUninvested ? "none" : formatNumber(alloc.quantity)}
      </td>
      <td className="px-3 py-3.5 font-mono text-sm font-medium tabular-nums text-text-hi md:px-4">
        {formatCurrency(row.valueUsd)}
      </td>
      <td className="hidden px-3 py-3.5 md:px-4 xl:table-cell">
        {isUninvested ? (
          <span className="text-xs text-text-mut font-mono">
            target {alloc.targetWeight.toFixed(0)}%
          </span>
        ) : (
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-1">
              <span className="text-xs text-text-lo font-mono w-10">
                {row.currentWeight.toFixed(1)}%
              </span>
              <span className="text-text-mut text-xs">vs</span>
              <span className="text-xs text-text-mut font-mono w-10">
                {alloc.targetWeight.toFixed(0)}%
              </span>
            </div>
            {row.driftAbs > 3 && (
              <Badge
                variant={row.driftAbs > 10 ? "danger" : "warning"}
                className="text-[10px] px-1.5 py-0"
              >
                {row.drift > 0 ? "+" : ""}
                {row.drift.toFixed(1)}%
              </Badge>
            )}
          </div>
        )}
      </td>
    </tr>
  );
}

function exposureSource(row: HoldingRow): string {
  const hasWallet = row.walletUsd > 0.005;
  const hasInvested = row.investedUsd > 0.005;
  if (hasWallet && hasInvested) return "wallet + invested";
  if (hasWallet) return "wallet";
  if (hasInvested) return "invested";
  return "target only";
}
