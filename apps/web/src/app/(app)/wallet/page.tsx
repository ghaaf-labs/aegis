"use client";

import { useState } from "react";
import { Copy, ExternalLink, Check, Wallet as WalletIcon } from "lucide-react";
import { motion } from "framer-motion";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";

/**
 * Dedicated wallet view. The Account dropdown shows addresses; this page
 * shows the live balances broken out per chain + per stable, with copy +
 * explorer affordances. Before this existed, the only place a user could
 * see their wallet values was a tiny "GATEWAY $X USDC · €Y EURC" string
 * in the header — easy to miss, and per-chain breakdown was discarded.
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
      <div className="max-w-3xl mx-auto py-12 text-center">
        <p className="text-sm font-mono text-text-mut">
          No wallet provisioned yet — finish signup to create one.
        </p>
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
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4 }}
      className="max-w-4xl mx-auto space-y-6"
    >
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight flex items-center gap-2">
          <WalletIcon className="w-5 h-5 text-accent-pnl" />
          Wallet
        </h1>
        {isEmpty && <FaucetButton />}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Total wallet balance</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-3xl font-bold text-text-hi">
            {formatCurrency(totalUsdEquivalent)}
          </p>
          <p className="text-xs font-mono text-text-mut mt-1">
            {formatCurrency(unifiedUsdc)} USDC · €{unifiedEurc.toFixed(2)} EURC
            {unifiedEurc > 0 && (
              <span className="text-text-lo">
                {" "}
                (≈ {formatCurrency(unifiedEurc * eurcUsd)})
              </span>
            )}
          </p>
          <p className="text-[11px] font-mono text-text-mut mt-3">
            Undeployed cash sits in Circle Gateway across the chains below. Use
            the &quot;Deploy wallet balance&quot; button on the Dashboard to
            allocate it across your target weights.
          </p>
        </CardContent>
      </Card>

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
    </motion.div>
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
  const [copied, setCopied] = useState(false);
  const total = usdc + eurc * eurcUsd;

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      /* clipboard API blocked — no-op */
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center justify-between gap-3">
          <span>{label}</span>
          <span className="text-sm font-mono text-accent-pnl tabular-nums">
            {formatCurrency(total)}
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
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
            Address
          </p>
          <div className="flex items-center gap-2">
            <code
              className="text-[11px] font-mono text-text-default truncate flex-1"
              title={address}
            >
              {address}
            </code>
            <button
              onClick={() => void handleCopy()}
              className="p-1 rounded-sharp hover:bg-white/5 text-text-lo hover:text-text-hi"
              title="Copy address"
              aria-label="Copy address"
            >
              {copied ? (
                <Check className="w-3.5 h-3.5 text-accent-pnl" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
            <a
              href={`${explorerBase}${address}`}
              target="_blank"
              rel="noreferrer"
              className="p-1 rounded-sharp hover:bg-white/5 text-text-lo hover:text-text-hi"
              title="View on explorer"
              aria-label="View on explorer"
            >
              <ExternalLink className="w-3.5 h-3.5" />
            </a>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
