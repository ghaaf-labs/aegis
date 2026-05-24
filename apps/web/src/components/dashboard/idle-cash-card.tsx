"use client";

import Link from "next/link";
import { Wallet, ArrowRight, CircleAlert, Loader2 } from "lucide-react";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { formatCurrency, timeAgo } from "@/lib/utils";
import { deriveCashSplit } from "@/lib/cash-model";
import { derivePortfolioPositionMetrics } from "@/lib/portfolio-values";
import { targetAllocationsForPortfolio } from "@/components/dashboard/target-allocations";
import {
  chainBalanceRows,
  walletRouteKeysFromNetworks,
} from "@/lib/wallet-routes";
import {
  BrutalCard as Card,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
  BrutalCardBody as CardContent,
  ProvenanceLine,
} from "@aegis/ui";

export function IdleCashCard() {
  const wallet = usePortfolioStore((s) => s.wallet);
  const portfolio = useActivePortfolio();
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const gatewayBalanceUpdatedAt = usePortfolioStore(
    (s) => s.gatewayBalanceUpdatedAt,
  );
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

  const investedUsd = derivePortfolioPositionMetrics(
    portfolio,
    snapshot,
  ).investedUsd;
  const cashSplit = deriveCashSplit({
    unifiedUsdc,
    unifiedEurc,
    targetAllocations: targetAllocationsForPortfolio(portfolio),
    investedUsd,
    snapshot,
  });
  const eurcUsd = cashSplit.eurcUsd;
  const totalUsd = cashSplit.totalWalletUsd;
  const hasIdleCash = totalUsd > 0.5;
  const balanceLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const balanceUnavailable = gatewayBalanceStatus === "error";
  const balanceRows = chainBalanceRows({
    perChainUsdc,
    perChainEurc,
    eurcUsd,
    routeKeys: walletRouteKeysFromNetworks(wallet?.networks),
  });
  const activeBalanceRows = balanceRows.filter((row) => row.totalUsd > 0.5);
  const showRouteBreakdown =
    !balanceUnavailable && !balanceLoading && hasIdleCash;
  const visibleBalanceRows = showRouteBreakdown ? activeBalanceRows : [];
  const hiddenEmptyRoutes = showRouteBreakdown
    ? Math.max(0, balanceRows.length - visibleBalanceRows.length)
    : 0;
  const cashLocation =
    activeBalanceRows.length === 1
      ? `on ${activeBalanceRows[0]?.shortLabel ?? "one route"}`
      : activeBalanceRows.length > 1
        ? `across ${activeBalanceRows.length} routes`
        : "in your wallet";

  return (
    <Card data-testid="idle-cash-card" className="flex min-h-[280px] flex-col">
      <CardHeader className="min-h-[52px] shrink-0">
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5 text-accent-pnl" />
          Wallet Balance
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col gap-3">
        <div>
          <p
            className={`font-mono font-bold leading-none tabular-nums ${
              balanceUnavailable
                ? "text-lg text-warn"
                : balanceLoading
                  ? "text-lg text-text-lo"
                  : "text-2xl text-text-hi sm:text-3xl"
            }`}
          >
            {balanceUnavailable
              ? "Balance unavailable"
              : balanceLoading
                ? "Checking wallet..."
                : formatCurrency(totalUsd)}
          </p>
          <p className="mt-3 min-h-8 font-mono text-[11px] leading-relaxed text-text-mut">
            {balanceUnavailable ? (
              (gatewayBalanceError ??
              "Aegis could not confirm the current wallet balance.")
            ) : balanceLoading ? (
              "Waiting for a current wallet balance."
            ) : hasIdleCash ? (
              <>
                {formatCurrency(unifiedUsdc, { compact: true })} USDC
                {unifiedEurc > 0 && (
                  <>
                    {" · "}
                    {unifiedEurc.toFixed(2)} EURC
                  </>
                )}{" "}
                {cashLocation}
              </>
            ) : (
              "Wallet is ready, but no idle cash is available"
            )}
          </p>
        </div>

        {balanceUnavailable && (
          <div className="flex items-start gap-2 border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-warn">
            <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              These are not confirmed zeros. Cash actions stay hidden until the
              balance check succeeds.
            </span>
          </div>
        )}

        {!balanceUnavailable &&
          !balanceLoading &&
          hasIdleCash &&
          cashSplit.hasUsdcReserveTarget && (
            <div className="grid gap-1.5 sm:grid-cols-2">
              <div className="border border-border-default bg-bg/70 px-2.5 py-2">
                <p className="font-mono text-[10px] uppercase tracking-wider text-text-mut">
                  USDC reserve · target {cashSplit.usdcTargetWeight.toFixed(0)}%
                </p>
                <p className="mt-0.5 font-mono text-sm font-semibold tabular-nums text-text-hi">
                  {formatCurrency(cashSplit.reserveUsd, { compact: true })}
                </p>
              </div>
              <div className="border border-accent-pnl/30 bg-accent-pnl/5 px-2.5 py-2">
                <p className="font-mono text-[10px] uppercase tracking-wider text-accent-pnl">
                  Deployable surplus
                </p>
                <p className="mt-0.5 font-mono text-sm font-semibold tabular-nums text-accent-pnl">
                  {formatCurrency(cashSplit.deployableUsd, { compact: true })}
                </p>
              </div>
            </div>
          )}

        {balanceLoading ? (
          <div className="flex min-h-12 items-center gap-2 border border-border-default bg-bg/70 px-3 py-2 font-mono text-[11px] text-text-lo">
            <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-accent-agent" />
            <span>Gateway balance sync in progress</span>
          </div>
        ) : !balanceUnavailable && !hasIdleCash ? (
          <div className="flex min-h-12 items-center gap-2 border border-border-default bg-bg/70 px-3 py-2 font-mono text-[11px] leading-relaxed text-text-lo">
            <Wallet className="h-3.5 w-3.5 shrink-0 text-text-mut" />
            <span>No USDC or EURC detected on active wallet routes.</span>
          </div>
        ) : (
          <div className="grid gap-1.5">
            {visibleBalanceRows.map((row) => (
              <div
                key={row.key}
                className="grid min-h-10 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border border-border-default bg-bg/70 px-2.5 py-2"
              >
                <div className="min-w-0">
                  <p className="truncate font-mono text-[10px] uppercase tracking-wider text-text-mut">
                    {row.shortLabel}
                  </p>
                  <p className="mt-0.5 truncate font-mono text-[10px] text-text-lo">
                    {balanceUnavailable ? (
                      "waiting for balance"
                    ) : (
                      <>
                        {row.usdc.toFixed(2)} USDC
                        {row.eurc > 0 && (
                          <>
                            {" · "}
                            {row.eurc.toFixed(2)} EURC
                          </>
                        )}
                      </>
                    )}
                  </p>
                </div>
                <p className="text-right text-sm font-semibold text-text-hi tabular-nums">
                  {balanceUnavailable
                    ? "—"
                    : formatCurrency(row.totalUsd, { compact: true })}
                </p>
              </div>
            ))}
            {hiddenEmptyRoutes > 0 && !balanceUnavailable && (
              <p className="border border-border-default bg-bg/50 px-2.5 py-2 font-mono text-[10px] text-text-mut">
                {hiddenEmptyRoutes} empty wallet{" "}
                {hiddenEmptyRoutes === 1 ? "route" : "routes"} hidden
              </p>
            )}
          </div>
        )}

        <Link
          href="/wallets"
          className="flex min-h-11 items-center justify-between gap-3 border border-accent-pnl/20 bg-accent-pnl/5 px-3 py-2 font-mono text-xs text-accent-pnl transition-colors hover:bg-accent-pnl/10"
        >
          <span className="min-w-0 truncate">
            {balanceUnavailable
              ? "Retry wallet"
              : hasIdleCash
                ? "Wallet details"
                : "Open wallet"}
          </span>
          {balanceLoading ? (
            <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin" />
          ) : (
            <ArrowRight className="h-3.5 w-3.5 shrink-0" />
          )}
        </Link>

        <div className="mt-auto border-t border-white/10 pt-2">
          <ProvenanceLine
            source={
              balanceUnavailable
                ? "balances as reported by Circle · check failed"
                : "balances as reported by Circle"
            }
            freshness={
              balanceUnavailable
                ? "needs retry"
                : gatewayBalanceUpdatedAt
                  ? `refreshed ${timeAgo(new Date(gatewayBalanceUpdatedAt).toISOString())}`
                  : "live"
            }
            className={balanceUnavailable ? "text-warn" : undefined}
          />
        </div>
      </CardContent>
    </Card>
  );
}
