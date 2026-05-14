"use client";

import { useState } from "react";
import { Coins } from "lucide-react";
import { BrutalButton, ProvenanceLine } from "@aegis/ui";
import { faucetApi, analyticsApi } from "@/lib/api";

export function FaucetButton() {
  const [submitting, setSubmitting] = useState(false);
  const [claimed, setClaimed] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const claim = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const r = await faucetApi.claim();
      setClaimed(r.amountUsdc);
      await analyticsApi.track("faucet.claimed", {
        amountUsdc: r.amountUsdc,
        chain: r.chain,
      });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  if (claimed !== null) {
    return (
      <div className="font-mono text-xs text-text-default flex items-center gap-2">
        <span className="text-accent-pnl">
          +${claimed.toFixed(2)} USDC claimed
        </span>
        <ProvenanceLine source="Circle Faucet · Arc testnet" />
      </div>
    );
  }

  return (
    <div className="flex items-center gap-3">
      <BrutalButton
        variant="agent"
        onClick={() => void claim()}
        disabled={submitting}
      >
        <Coins className="w-3.5 h-3.5" />
        {submitting ? "Claiming…" : "Get testnet USDC"}
      </BrutalButton>
      <ProvenanceLine source="Circle Faucet · Arc testnet · 100 USDC/24h" />
      {error && <span className="text-xs text-risk font-mono">{error}</span>}
    </div>
  );
}
