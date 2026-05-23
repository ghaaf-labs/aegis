"use client";

import Link from "next/link";
import {
  ArrowRight,
  CircleAlert,
  ClipboardCheck,
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

export function AssetTable() {
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
  if (!priceMap.USYC) {
    priceMap.USYC = {
      symbol: "USYC",
      priceUsd: 1.0,
      change24h: 0,
      change7d: 0,
      marketCap: 0,
      volume24h: 0,
      updatedAt: snapshot?.capturedAt ?? new Date().toISOString(),
    };
  }
  const walletCashKnown = gatewayBalanceStatus === "ready";
  const walletCashUnavailable = gatewayBalanceStatus === "error";
  const walletCashUsd = walletCashKnown ? unifiedUsdc : 0;

  const allocList = targetAllocationsForPortfolio(portfolio);
  const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const valueBySymbol = Object.fromEntries(
    metrics.positions.map((position) => [position.symbol, position]),
  );
  const isUninvested = metrics.investedUsd < 0.5;
  const hasWalletCash = isUninvested && walletCashUsd > 0.5;
  const hasUsdcSleeve = allocList.some(
    (a) => a.symbol === "USDC" && a.targetWeight > 0,
  );
  return (
    <Card>
      <CardHeader>
        <CardTitle>
          {hasWalletCash
            ? "After Approval"
            : isUninvested
              ? "Target Mix"
              : "Current Holdings"}
        </CardTitle>
      </CardHeader>
      {hasWalletCash && (
        <div className="mx-5 mb-4 border-brutal border-accent-pnl/40 bg-accent-pnl/5 p-3 rounded-sharp">
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
                  {unifiedEurc > 0 &&
                    " Existing EURC cash stays in Wallet until you approve a move for it."}
                </p>
              </div>
            </div>
            <Link
              href={`/dashboard/${portfolio.id}`}
              className="inline-flex shrink-0 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-pnl px-3 py-2 text-xs font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal transition-[box-shadow]"
            >
              Review plan
              <ArrowRight className="h-3.5 w-3.5" />
            </Link>
          </div>
        </div>
      )}
      {walletCashUnavailable && (
        <div className="mx-5 mb-4 border-brutal border-warn/50 bg-warn/5 p-3 rounded-sharp">
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
      <CardContent className="p-0">
        {hasWalletCash ? (
          <div className="grid gap-2 px-5 pb-5 sm:grid-cols-2 xl:grid-cols-3">
            {allocList.map((alloc) => {
              const plannedUsd = walletCashUsd * (alloc.targetWeight / 100);
              const isUsdcReserve = alloc.symbol === "USDC";
              return (
                <div
                  key={alloc.symbol}
                  className="rounded-sharp border border-border-default bg-surface p-3 font-mono"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2">
                      <span className="inline-flex h-7 min-w-10 items-center justify-center rounded-sharp border border-accent-agent/35 bg-accent-agent/10 px-2 text-[10px] font-semibold text-text-hi">
                        {alloc.symbol}
                      </span>
                      <div className="min-w-0">
                        <p className="truncate text-sm font-semibold text-text-hi">
                          {isUsdcReserve
                            ? "Cash reserve"
                            : `${alloc.symbol} target`}
                        </p>
                        <p className="text-[10px] uppercase tracking-wider text-text-mut">
                          {alloc.targetWeight.toFixed(0)}% target
                        </p>
                      </div>
                    </div>
                    <ClipboardCheck className="h-4 w-4 shrink-0 text-accent-agent/70" />
                  </div>
                  <div className="mt-4">
                    <p className="text-xl font-semibold tabular-nums text-text-hi">
                      {formatCurrency(plannedUsd)}
                    </p>
                    <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                      {isUsdcReserve
                        ? "Held as reserve cash."
                        : "Ready after approval."}
                    </p>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <table className="w-full table-fixed">
            <thead>
              <tr className="border-b border-white/5">
                {(
                  [
                    ["Asset", ""],
                    ["Price", ""],
                    ["24h", "hidden 2xl:table-cell"],
                    ["Holdings", "hidden 2xl:table-cell"],
                    ["Value", ""],
                    ["Weight vs Target", "hidden 2xl:table-cell"],
                  ] as const
                ).map(([h, cls]) => (
                  <th
                    key={h}
                    className={
                      "px-3 py-3 text-left text-[11px] font-medium text-text-mut uppercase tracking-wider md:px-4 " +
                      cls
                    }
                  >
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {allocList.map((alloc, i) => {
                const price = priceMap[alloc.symbol];
                const position = valueBySymbol[alloc.symbol];
                const currentWeight = position?.currentWeight ?? 0;
                const liveValueUsd = (price?.priceUsd ?? 0) * alloc.quantity;
                const valueUsd =
                  position?.valueUsd ??
                  (liveValueUsd > 0 ? liveValueUsd : alloc.valueUsd);
                const fallbackPriceUsd =
                  alloc.quantity > 0 && alloc.valueUsd > 0
                    ? alloc.valueUsd / alloc.quantity
                    : null;
                const displayPriceUsd = price?.priceUsd ?? fallbackPriceUsd;
                const drift = currentWeight - alloc.targetWeight;
                const driftAbs = Math.abs(drift);

                return (
                  <tr
                    key={alloc.symbol}
                    className={`border-b border-white/3 hover:bg-white/2 transition-colors ${
                      i === allocList.length - 1 ? "border-0" : ""
                    }`}
                  >
                    <td className="px-3 py-3.5 md:px-4">
                      <div className="flex items-center gap-2.5">
                        <div className="flex h-7 min-w-10 items-center justify-center rounded-sharp border border-accent-agent/30 bg-accent-agent/10 px-1.5">
                          <span className="text-[9px] font-bold text-text-hi">
                            {alloc.symbol}
                          </span>
                        </div>
                        <span className="text-sm font-semibold text-text-hi font-mono">
                          {alloc.symbol}
                        </span>
                      </div>
                    </td>
                    <>
                      <td
                        className={`px-3 py-3.5 text-sm font-medium md:px-4 ${priceColor}`}
                      >
                        {displayPriceUsd
                          ? formatCurrency(displayPriceUsd)
                          : "—"}
                      </td>
                      <td className="hidden px-3 py-3.5 md:px-4 2xl:table-cell">
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
                      <td className="hidden px-3 py-3.5 text-xs font-mono text-text-lo md:px-4 2xl:table-cell">
                        {isUninvested ? "none" : formatNumber(alloc.quantity)}
                      </td>
                      <td className="px-3 py-3.5 text-sm font-medium text-text-hi md:px-4">
                        {formatCurrency(valueUsd)}
                      </td>
                      <td className="hidden px-3 py-3.5 md:px-4 2xl:table-cell">
                        {isUninvested ? (
                          <span className="text-xs text-text-mut font-mono">
                            target {alloc.targetWeight.toFixed(0)}%
                          </span>
                        ) : (
                          <div className="flex items-center gap-2">
                            <div className="flex items-center gap-1">
                              <span className="text-xs text-text-lo font-mono w-10">
                                {currentWeight.toFixed(1)}%
                              </span>
                              <span className="text-text-mut text-xs">vs</span>
                              <span className="text-xs text-text-mut font-mono w-10">
                                {alloc.targetWeight.toFixed(0)}%
                              </span>
                            </div>
                            {driftAbs > 3 && (
                              <Badge
                                variant={driftAbs > 10 ? "danger" : "warning"}
                                className="text-[10px] px-1.5 py-0"
                              >
                                {drift > 0 ? "+" : ""}
                                {drift.toFixed(1)}%
                              </Badge>
                            )}
                          </div>
                        )}
                      </td>
                    </>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
        <div className="px-5 py-2 text-[10px] text-text-mut font-mono border-t border-white/5">
          {hasWalletCash
            ? hasUsdcSleeve
              ? "USDC target stays as reserve cash; other targets wait for approval"
              : "Targets wait for approval before funds move"
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
