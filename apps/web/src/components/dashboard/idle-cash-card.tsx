"use client";

import Link from "next/link";
import { Wallet, ArrowRight, CircleAlert, Loader2 } from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";
import { ProvenanceLine } from "@aegis/ui";

export function IdleCashCard() {
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

  const arcTotal = (perChainUsdc.arc ?? 0) + (perChainEurc.arc ?? 0) * eurcUsd;
  const baseTotal =
    (perChainUsdc.base ?? 0) + (perChainEurc.base ?? 0) * eurcUsd;

  return (
    <Card data-testid="idle-cash-card" className="h-full">
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
              available
            </>
          ) : (
            "No wallet cash available"
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

        <div className="grid grid-cols-2 gap-2">
          <div className="border border-border-default bg-bg/70 p-2">
            <p className="mb-1 font-mono text-[10px] uppercase tracking-wider text-text-mut">
              Arc
            </p>
            <p className="text-sm font-semibold text-text-hi tabular-nums">
              {balanceUnavailable || balanceLoading
                ? "—"
                : formatCurrency(arcTotal, { compact: true })}
            </p>
            <p className="mt-0.5 font-mono text-[10px] text-text-lo">
              {balanceUnavailable || balanceLoading ? (
                "pending check"
              ) : (
                <>
                  {(perChainUsdc.arc ?? 0).toFixed(2)} USDC
                  {(perChainEurc.arc ?? 0) > 0 && (
                    <> · €{(perChainEurc.arc ?? 0).toFixed(2)}</>
                  )}
                </>
              )}
            </p>
          </div>
          <div className="border border-border-default bg-bg/70 p-2">
            <p className="mb-1 font-mono text-[10px] uppercase tracking-wider text-text-mut">
              Base
            </p>
            <p className="text-sm font-semibold text-text-hi tabular-nums">
              {balanceUnavailable || balanceLoading
                ? "—"
                : formatCurrency(baseTotal, { compact: true })}
            </p>
            <p className="mt-0.5 font-mono text-[10px] text-text-lo">
              {balanceUnavailable || balanceLoading ? (
                "pending check"
              ) : (
                <>
                  {(perChainUsdc.base ?? 0).toFixed(2)} USDC
                  {(perChainEurc.base ?? 0) > 0 && (
                    <> · €{(perChainEurc.base ?? 0).toFixed(2)}</>
                  )}
                </>
              )}
            </p>
          </div>
        </div>

        <Link
          href="/wallets"
          className="flex min-h-10 items-center justify-between gap-3 border border-accent-pnl/20 bg-accent-pnl/5 px-3 py-2 font-mono text-xs text-accent-pnl transition-colors hover:bg-accent-pnl/10"
        >
          <span className="min-w-0 truncate">
            {balanceUnavailable
              ? "Retry in wallet"
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
