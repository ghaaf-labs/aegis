"use client";

import Link from "next/link";
import {
  ArrowRight,
  CircleAlert,
  TrendingUp,
  TrendingDown,
  Wallet,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import {
  formatCurrency,
  formatPercent,
  formatNumber,
  changeColor,
} from "@/lib/utils";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";

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
  const deployableUsd = walletCashKnown ? unifiedUsdc : 0;

  const allocList = portfolio.allocations ?? [];
  const metrics = derivePortfolioPositionMetrics(portfolio, snapshot);
  const valueBySymbol = Object.fromEntries(
    metrics.positions.map((position) => [position.symbol, position]),
  );
  const isUninvested = metrics.investedUsd < 0.5;
  const hasDeployableWallet = isUninvested && deployableUsd > 0.5;
  const hasUsdcSleeve = allocList.some(
    (a) => a.symbol === "USDC" && a.targetWeight > 0,
  );
  return (
    <Card>
      <CardHeader>
        <CardTitle>
          {hasDeployableWallet
            ? "Target Deployment Plan"
            : isUninvested
              ? "Target Positions"
              : "Invested Positions"}
        </CardTitle>
      </CardHeader>
      {hasDeployableWallet && (
        <div className="mx-5 mb-4 border-brutal border-accent-pnl/40 bg-accent-pnl/5 p-3 rounded-sharp">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-start gap-2">
              <Wallet className="mt-0.5 h-4 w-4 shrink-0 text-accent-pnl" />
              <div>
                <p className="text-sm font-mono font-semibold text-text-hi">
                  {formatCurrency(deployableUsd)} USDC is still in your wallet,
                  not invested yet.
                </p>
                <p className="mt-1 text-xs font-mono text-text-lo leading-relaxed">
                  The rows below show the planned outcome for deployable USDC
                  after approval. Non-USDC rows require execution; any USDC
                  target stays as cash reserve and needs no swap.
                  {unifiedEurc > 0 &&
                    " Existing EURC wallet cash is shown in Wallet until an approved StableFX leg confirms portfolio exposure."}
                </p>
              </div>
            </div>
            <Link
              href={`/dashboard/${portfolio.id}`}
              className="inline-flex shrink-0 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-pnl px-3 py-2 text-xs font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal transition-[box-shadow]"
            >
              Deploy from dashboard
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
                  "Circle Gateway did not confirm Arc + Base balances."}{" "}
                This table is showing target or invested positions only; it will
                not calculate a deployment outcome from stale wallet cash.
              </p>
            </div>
          </div>
        </div>
      )}
      <CardContent className="p-0">
        <table className="w-full table-fixed">
          <thead>
            <tr className="border-b border-white/5">
              {(hasDeployableWallet
                ? ([
                    ["Asset", ""],
                    ["Target", ""],
                    ["Planned outcome", ""],
                    ["Current holdings", "hidden 2xl:table-cell"],
                    ["Status", "hidden 2xl:table-cell"],
                  ] as const)
                : ([
                    ["Asset", ""],
                    ["Price", ""],
                    ["24h", "hidden 2xl:table-cell"],
                    ["Holdings", "hidden 2xl:table-cell"],
                    ["Value", ""],
                    ["Weight vs Target", "hidden 2xl:table-cell"],
                  ] as const)
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
            {(portfolio.allocations ?? []).map((alloc, i) => {
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
              const plannedUsd = deployableUsd * (alloc.targetWeight / 100);
              const isUsdcReserve =
                hasDeployableWallet && alloc.symbol === "USDC";
              const drift = currentWeight - alloc.targetWeight;
              const driftAbs = Math.abs(drift);

              return (
                <tr
                  key={alloc.symbol}
                  className={`border-b border-white/3 hover:bg-white/2 transition-colors ${
                    i === (portfolio.allocations?.length ?? 0) - 1
                      ? "border-0"
                      : ""
                  }`}
                >
                  <td className="px-3 py-3.5 md:px-4">
                    <div className="flex items-center gap-2.5">
                      <div className="w-7 h-7 rounded-sharp bg-accent-agent/10 flex items-center justify-center border border-accent-agent/30">
                        <span className="text-[10px] font-bold text-text-hi">
                          {alloc.symbol[0]}
                        </span>
                      </div>
                      <span className="text-sm font-semibold text-text-hi font-mono">
                        {alloc.symbol}
                      </span>
                    </div>
                  </td>
                  {hasDeployableWallet ? (
                    <>
                      <td className="px-3 py-3.5 text-sm font-mono text-text-hi tabular-nums md:px-4">
                        {alloc.targetWeight.toFixed(0)}%
                      </td>
                      <td className="px-3 py-3.5 text-sm font-semibold text-accent-pnl tabular-nums md:px-4">
                        {formatCurrency(plannedUsd)}
                        {isUsdcReserve && (
                          <span className="mt-0.5 block text-[10px] font-normal text-text-mut">
                            stays USDC
                          </span>
                        )}
                      </td>
                      <td className="hidden px-3 py-3.5 text-xs font-mono text-text-lo md:px-4 2xl:table-cell">
                        {isUninvested ? "none" : formatNumber(alloc.quantity)}
                      </td>
                      <td className="hidden px-3 py-3.5 md:px-4 2xl:table-cell">
                        <span className="text-xs text-text-mut font-mono">
                          {isUsdcReserve ? "cash reserve" : "awaiting approval"}
                        </span>
                      </td>
                    </>
                  ) : (
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
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
        <div className="px-5 py-2 text-[10px] text-text-mut font-mono border-t border-white/5">
          {hasDeployableWallet
            ? hasUsdcSleeve
              ? "USDC target weight is kept as reserve cash; non-USDC planned values use deployable Gateway USDC × target weights"
              : "Planned values use deployable Circle Gateway USDC × target weights"
            : walletCashUnavailable
              ? "Gateway wallet cash unavailable; deployment values are hidden until Circle confirms balances"
              : !walletCashKnown
                ? "Waiting for Gateway cash before calculating deployment values"
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
