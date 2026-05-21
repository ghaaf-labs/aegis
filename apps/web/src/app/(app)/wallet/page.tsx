"use client";

import { useState } from "react";
import { Copy, ExternalLink, Check, Wallet as WalletIcon } from "lucide-react";
import Link from "next/link";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";

/**
 * Dedicated wallet view — per-chain USDC + EURC balances with copy +
 * explorer affordances. Before this page the only balance surface was a
 * tiny "GATEWAY $X" string in the header.
 */
export default function WalletPage() {
  const wallet = usePortfolioStore((s) => s.wallet);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);

  const eurcUsd =
    snapshot?.assets.find((a) => a.symbol === "EURC")?.priceUsd ?? 1.085;
  const totalUsdEquivalent = unifiedUsdc + unifiedEurc * eurcUsd;

  if (!wallet) {
    return (
      <div className="py-12 text-center space-y-3">
        <p className="text-sm font-mono text-text-hi">
          No wallet is attached to this session.
        </p>
        <p className="text-xs font-mono text-text-mut max-w-md mx-auto leading-relaxed">
          If you just refreshed, wait a moment for auth to hydrate. Otherwise
          sign in with the same email, or create a wallet-backed account before
          using Gateway balances, deployment, or rebalance execution.
        </p>
        <div className="flex flex-wrap items-center justify-center gap-2">
          <Link
            href="/login"
            className="inline-flex px-3 py-1.5 border-2 border-accent-agent bg-accent-agent text-black text-xs font-semibold"
          >
            Sign in
          </Link>
          <Link
            href="/signup"
            className="inline-flex px-3 py-1.5 border-2 border-border-default text-text-lo text-xs font-semibold hover:border-border-hi hover:text-text-hi"
          >
            Create wallet
          </Link>
        </div>
      </div>
    );
  }

  const chains: Array<{
    key: "arc" | "base";
    label: string;
    address: string;
    explorerBase: string;
  }> = [
    {
      key: "arc",
      label: "Arc Testnet",
      address: wallet.arcAddress,
      explorerBase: "https://sepolia.arkscan.io/address/",
    },
    {
      key: "base",
      label: "Base Sepolia",
      address: wallet.baseAddress,
      explorerBase: "https://sepolia.basescan.org/address/",
    },
  ];

  const isEmpty = unifiedUsdc < 0.01 && unifiedEurc < 0.01;

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight flex items-center gap-2">
            <WalletIcon className="w-5 h-5 text-accent-pnl" />
            Wallets
          </h1>
          <p className="text-sm text-text-lo mt-1">
            Idle Circle Gateway cash only. Invested positions stay on Dashboard
            and Portfolio.
          </p>
        </div>
        {isEmpty && <FaucetButton />}
      </div>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">
            Idle wallet cash
          </span>
        </BrutalCardHeader>
        <BrutalCardBody>
          <p className="text-2xl font-mono font-semibold text-accent-pnl tabular-nums">
            {formatCurrency(totalUsdEquivalent)}
          </p>
          <p className="text-xs font-mono text-text-lo mt-1">
            {formatCurrency(unifiedUsdc)} USDC · €{unifiedEurc.toFixed(2)} EURC
            {unifiedEurc > 0 && (
              <span className="text-text-mut">
                {" "}
                (≈ {formatCurrency(unifiedEurc * eurcUsd)})
              </span>
            )}
          </p>
          <p className="text-[11px] font-mono text-text-mut mt-3">
            {isEmpty
              ? "This can be $0 even when you own investments. Deployed positions are counted on Dashboard and Portfolio; newly funded USDC appears here first."
              : "This is spendable cash that has not been invested yet. Review any deployment or rebalance plan before real execution."}
          </p>
        </BrutalCardBody>
      </BrutalCard>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {chains.map((c) => (
          <ChainCard
            key={c.key}
            label={c.label}
            address={c.address}
            explorerBase={c.explorerBase}
            usdc={perChainUsdc[c.key] ?? 0}
            eurc={perChainEurc[c.key] ?? 0}
            eurcUsd={eurcUsd}
          />
        ))}
      </div>
    </div>
  );
}

interface ChainCardProps {
  label: string;
  address: string;
  explorerBase: string;
  usdc: number;
  eurc: number;
  eurcUsd: number;
}

function ChainCard({
  label,
  address,
  explorerBase,
  usdc,
  eurc,
  eurcUsd,
}: ChainCardProps) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">(
    "idle",
  );
  const total = usdc + eurc * eurcUsd;

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopyState("copied");
      setTimeout(() => setCopyState("idle"), 1800);
    } catch {
      setCopyState("failed");
      setTimeout(() => setCopyState("idle"), 2200);
    }
  };

  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-sm font-mono text-text-hi">{label}</span>
        <span className="text-sm font-mono text-accent-pnl tabular-nums">
          {formatCurrency(total)}
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
              USDC
            </p>
            <p className="text-sm font-mono text-text-hi tabular-nums">
              {formatCurrency(usdc)}
            </p>
          </div>
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
              EURC
            </p>
            <p className="text-sm font-mono text-text-hi tabular-nums">
              €{eurc.toFixed(2)}
            </p>
          </div>
        </div>

        <div>
          <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
            Funding address
          </p>
          <div className="grid gap-2">
            <code
              className="block min-w-0 rounded-sharp border border-border-default bg-bg px-2 py-2 text-[11px] font-mono text-text-default break-all"
              title={address}
            >
              {address}
            </code>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => void handleCopy()}
                className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-white/10 bg-white/5 px-3 text-xs font-mono text-text-default hover:border-accent-pnl/40 hover:text-accent-pnl"
                title="Copy address"
                aria-label={`Copy ${label} address`}
              >
                {copyState === "copied" ? (
                  <Check className="w-3.5 h-3.5 text-accent-pnl" />
                ) : (
                  <Copy className="w-3.5 h-3.5" />
                )}
                {copyState === "copied"
                  ? "Copied"
                  : copyState === "failed"
                    ? "Copy failed"
                    : "Copy"}
              </button>
              <a
                href={`${explorerBase}${address}`}
                target="_blank"
                rel="noreferrer"
                className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-white/10 bg-white/5 px-3 text-xs font-mono text-text-default hover:border-accent-agent/40 hover:text-accent-agent"
                title="View on explorer"
                aria-label={`View ${label} on explorer`}
              >
                <ExternalLink className="w-3.5 h-3.5" />
                Explorer
              </a>
            </div>
          </div>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}
