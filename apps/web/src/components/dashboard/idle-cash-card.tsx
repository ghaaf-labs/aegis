"use client";

import Link from "next/link";
import { Wallet, ArrowRight, CircleAlert, Loader2 } from "lucide-react";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";
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
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

  const eurcUsd =
    snapshot?.assets.find((a) => a.symbol === "EURC")?.priceUsd ?? 1.085;
  const totalUsd = unifiedUsdc + unifiedEurc * eurcUsd;
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
  const visibleBalanceRows =
    activeBalanceRows.length > 0 ? activeBalanceRows : balanceRows.slice(0, 2);
  const hiddenEmptyRoutes = Math.max(
    0,
    balanceRows.length - visibleBalanceRows.length,
  );
  const cashLocation =
    activeBalanceRows.length === 1
      ? `on ${activeBalanceRows[0]?.shortLabel ?? "one route"}`
      : `across ${activeBalanceRows.length} routes`;

  return (
    <Card data-testid="idle-cash-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wallet className="w-3.5 h-3.5 text-accent-pnl" />
          Wallet Balance
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <p
          className={`font-bold leading-none ${
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
        <p className="font-mono text-[11px] text-text-mut">
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
                  {" · "}€{unifiedEurc.toFixed(2)} EURC
                </>
              )}{" "}
              {cashLocation}
            </>
          ) : (
            "Wallet is ready, but no idle cash is available"
          )}
        </p>

        {balanceUnavailable && (
          <div className="flex items-start gap-2 border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-warn">
            <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              These are not confirmed zeros. Cash actions stay hidden until the
              balance check succeeds.
            </span>
          </div>
        )}

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
                  {balanceUnavailable || balanceLoading ? (
                    "waiting for balance"
                  ) : (
                    <>
                      {row.usdc.toFixed(2)} USDC
                      {row.eurc > 0 && <> · €{row.eurc.toFixed(2)}</>}
                    </>
                  )}
                </p>
              </div>
              <p className="text-right text-sm font-semibold text-text-hi tabular-nums">
                {balanceUnavailable || balanceLoading
                  ? "—"
                  : formatCurrency(row.totalUsd, { compact: true })}
              </p>
            </div>
          ))}
          {hiddenEmptyRoutes > 0 && !balanceLoading && !balanceUnavailable && (
            <p className="border border-border-default bg-bg/50 px-2.5 py-2 font-mono text-[10px] text-text-mut">
              {hiddenEmptyRoutes} empty wallet{" "}
              {hiddenEmptyRoutes === 1 ? "route" : "routes"} hidden
            </p>
          )}
        </div>

        <Link
          href="/wallets"
          className="flex min-h-10 items-center justify-between gap-3 border border-accent-pnl/20 bg-accent-pnl/5 px-3 py-2 font-mono text-xs text-accent-pnl transition-colors hover:bg-accent-pnl/10"
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

        <div className="border-t border-white/10 pt-2">
          <ProvenanceLine
            source={
              balanceUnavailable
                ? "wallet balance service · check failed"
                : "wallet balance service · current total"
            }
            freshness={balanceUnavailable ? "needs retry" : "live"}
            className={balanceUnavailable ? "text-warn" : undefined}
          />
        </div>
      </CardContent>
    </Card>
  );
}
