"use client";

import { cn } from "@/lib/utils";
import { ChainBadge } from "@aegis/ui";
import type { ChainKey, LegStatus } from "@/types";
import { explorerTxUrl } from "@/lib/explorers";

interface LegCardProps {
  legIndex: number;
  kind: string;
  srcChain?: ChainKey | null;
  destChain?: ChainKey | null;
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
  pending: "bg-gray-800 text-text-lo",
  submitted: "bg-cyan-500/20 text-accent-agent animate-pulse",
  confirmed: "bg-cyan-500/20 text-accent-agent",
  failed: "bg-rose-500/20 text-risk",
};

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
  // A CCTP burn executes on the source chain; the mint and single-chain legs
  // execute on the destination, so link each tx to the chain it actually ran on.
  const explorerChain =
    kind === "cross_chain_burn"
      ? (srcChain ?? destChain)
      : (destChain ?? srcChain);
  const explorer = explorerTxUrl(explorerChain, txHash);
  return (
    <div
      data-testid="leg-card"
      className="border-2 border-white/10 bg-[#141414] p-4 flex items-start gap-4"
    >
      <div className="font-mono text-xs text-text-mut w-6 mt-1">
        {String(legIndex + 1).padStart(2, "0")}
      </div>
      <div className="flex-1">
        <div className="flex items-center gap-2 mb-1">
          <span className="text-sm font-semibold text-text-hi">
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
            <span className="text-[10px] px-1.5 py-0.5 bg-cyan-500/10 border border-cyan-500/30 text-accent-agent font-mono tracking-wider">
              Hook executed
            </span>
          )}
          {kind === "cross_chain_burn" && status === "confirmed" && (
            <span className="text-[10px] px-1.5 py-0.5 bg-cyan-500/10 border border-cyan-500/30 text-accent-agent font-mono tracking-wider">
              Hook payload sent
            </span>
          )}
        </div>
        <div className="text-xs text-text-lo font-mono flex items-center gap-2 flex-wrap">
          {srcSymbol ?? "?"}
          {srcChain && <ChainBadge chain={toChainBadge(srcChain)} />}
          <span className="text-text-mut">→</span>
          {destSymbol ?? "?"}
          {destChain && <ChainBadge chain={toChainBadge(destChain)} />}
          <span className="ml-2 text-accent-agent">
            ${amountUsdc.toFixed(2)}
          </span>
        </div>
        {explorer && (
          <a
            className="mt-2 inline-block text-[11px] font-mono text-accent-agent hover:text-accent-agent underline"
            href={explorer}
            target="_blank"
            rel="noreferrer"
          >
            view on explorer ↗
          </a>
        )}
        {failureReason && (
          <p className="mt-2 text-[11px] text-risk font-mono">
            {failureReason}
          </p>
        )}
      </div>
    </div>
  );
}

function toChainBadge(chain: ChainKey): "ARC" | "BASE" {
  return chain === "arc" ? "ARC" : "BASE";
}
