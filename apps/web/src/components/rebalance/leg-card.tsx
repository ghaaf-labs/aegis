"use client";

import { cn } from "@/lib/utils";
import { ChainBadge } from "@aegis/ui";
import type { LegStatus } from "@/types";

interface LegCardProps {
  legIndex: number;
  kind: string;
  srcChain?: string | null;
  destChain?: string | null;
  srcSymbol?: string | null;
  destSymbol?: string | null;
  amountUsdc: number;
  status: LegStatus;
  txHash?: string | null;
  failureReason?: string | null;
}

const KIND_LABEL: Record<string, string> = {
  local_swap: "Swap on chain",
  cross_chain_burn: "CCTP V2 burn",
  cross_chain_mint: "CCTP V2 mint + hook",
  park_usyc: "Park into USYC",
  redeem_usyc: "Redeem USYC",
  fx_stablefx: "StableFX (USDC ↔ EURC)",
};

const STATUS_CLASSES: Record<LegStatus, string> = {
  pending: "bg-gray-800 text-gray-400",
  submitted: "bg-cyan-500/20 text-cyan-300 animate-pulse",
  confirmed: "bg-cyan-500/20 text-cyan-300",
  failed: "bg-rose-500/20 text-rose-300",
};

function explorerUrl(
  chain: string | null | undefined,
  tx: string | null | undefined,
) {
  if (!chain || !tx) return null;
  if (chain === "base") return `https://sepolia.basescan.org/tx/${tx}`;
  if (chain === "arc") return `https://explorer.testnet.arc.network/tx/${tx}`;
  return null;
}

export function LegCard({
  legIndex,
  kind,
  srcChain,
  destChain,
  srcSymbol,
  destSymbol,
  amountUsdc,
  status,
  txHash,
  failureReason,
}: LegCardProps) {
  const explorer = explorerUrl(destChain ?? srcChain, txHash);
  return (
    <div className="border-2 border-white/10 bg-[#141414] p-4 flex items-start gap-4">
      <div className="font-mono text-xs text-gray-500 w-6 mt-1">
        {String(legIndex + 1).padStart(2, "0")}
      </div>
      <div className="flex-1">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-sm font-semibold text-white">
            {KIND_LABEL[kind] ?? kind}
          </span>
          <span
            className={cn(
              "uppercase text-[10px] font-mono tracking-wider px-2 py-0.5",
              STATUS_CLASSES[status],
            )}
          >
            {status}
          </span>
          {kind === "cross_chain_mint" && status === "confirmed" && (
            <span className="text-[10px] px-1.5 py-0.5 bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 font-mono tracking-wider">
              Hook executed
            </span>
          )}
          {kind === "cross_chain_burn" && status === "confirmed" && (
            <span className="text-[10px] px-1.5 py-0.5 bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 font-mono tracking-wider">
              Hook payload sent
            </span>
          )}
        </div>
        <div className="text-xs text-gray-400 font-mono flex items-center gap-2 flex-wrap">
          {srcSymbol ?? "?"}
          {srcChain && (
            <ChainBadge
              chain={srcChain.toUpperCase() as "ARC" | "BASE" | "AVAX"}
            />
          )}
          <span className="text-gray-600">→</span>
          {destSymbol ?? "?"}
          {destChain && (
            <ChainBadge
              chain={destChain.toUpperCase() as "ARC" | "BASE" | "AVAX"}
            />
          )}
          <span className="ml-2 text-cyan-400">${amountUsdc.toFixed(2)}</span>
        </div>
        {explorer && (
          <a
            className="mt-2 inline-block text-[11px] font-mono text-cyan-400 hover:text-cyan-300 underline"
            href={explorer}
            target="_blank"
            rel="noreferrer"
          >
            view on explorer ↗
          </a>
        )}
        {failureReason && (
          <p className="mt-2 text-[11px] text-rose-300 font-mono">
            {failureReason}
          </p>
        )}
      </div>
    </div>
  );
}
