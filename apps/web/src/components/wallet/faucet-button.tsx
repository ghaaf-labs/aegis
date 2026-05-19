"use client";

import { useState } from "react";
import { Coins, ExternalLink, Copy, Check } from "lucide-react";
import { BrutalButton, ProvenanceLine } from "@aegis/ui";
import { faucetApi, analyticsApi, type FaucetClaim } from "@/lib/api";

export function FaucetButton() {
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<FaucetClaim | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const claim = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const r = await faucetApi.claim();
      setResult(r);
      await analyticsApi.track("faucet.claimed", {
        amountUsdc: r.amountUsdc,
        chain: r.chain,
      });
      // In real mode the backend hands back the public Circle faucet URL —
      // pop it open in a new tab so the user can finish the claim there.
      if (r.claimUrl && typeof window !== "undefined") {
        window.open(r.claimUrl, "_blank", "noopener,noreferrer");
      }
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  const copyAddress = async () => {
    if (!result?.arcAddress) return;
    await navigator.clipboard.writeText(result.arcAddress);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  if (result) {
    // Real mode: show the address + a "Open Circle faucet" link so the user
    // can paste and complete the claim.
    if (result.claimUrl) {
      return (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <a
              href={result.claimUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 text-xs font-mono text-accent-agent hover:underline"
            >
              <ExternalLink className="w-3 h-3" />
              Open Circle Faucet
            </a>
            <button
              type="button"
              onClick={() => void copyAddress()}
              className="inline-flex items-center gap-1 text-[11px] font-mono text-text-lo hover:text-text-hi"
              title="Copy ARC address"
            >
              {copied ? (
                <Check className="w-3 h-3 text-accent-pnl" />
              ) : (
                <Copy className="w-3 h-3" />
              )}
              {result.arcAddress?.slice(0, 8)}…{result.arcAddress?.slice(-4)}
            </button>
          </div>
          <p className="text-[11px] text-text-mut font-mono">
            Paste the address above into Circle&apos;s faucet, select Arc
            Sepolia, and claim. Balance refreshes within ~30s.
          </p>
        </div>
      );
    }
    // Mock mode: synthetic balance applied.
    return (
      <div className="font-mono text-xs text-text-default flex items-center gap-2">
        <span className="text-accent-pnl">
          +${result.amountUsdc.toFixed(2)} USDC claimed
        </span>
        <ProvenanceLine source="Mock faucet" />
      </div>
    );
  }

  return (
    <div data-testid="faucet-button" className="flex items-center gap-3">
      <BrutalButton
        variant="agent"
        onClick={() => void claim()}
        disabled={submitting}
      >
        <Coins className="w-3.5 h-3.5" />
        {submitting ? "Claiming…" : "Get testnet USDC"}
      </BrutalButton>
      <ProvenanceLine source="Circle Faucet · Arc Sepolia · 100 USDC/24h" />
      {error && <span className="text-xs text-risk font-mono">{error}</span>}
    </div>
  );
}
