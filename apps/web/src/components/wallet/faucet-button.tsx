"use client";

import { useState } from "react";
import { Coins, ExternalLink, Copy, Check } from "lucide-react";
import { BrutalButton, ProvenanceLine } from "@aegis/ui";
import { faucetApi, analyticsApi, type FaucetClaim } from "@/lib/api";
import { copyTextToClipboard } from "@/lib/clipboard";

export function FaucetButton() {
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<FaucetClaim | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">(
    "idle",
  );
  const showFaucetLink = error?.includes("already requested") ?? false;

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
      // In real mode the backend hands back the public faucet URL —
      // pop it open in a new tab so the user can finish the claim there.
      if (r.claimUrl && typeof window !== "undefined") {
        window.open(r.claimUrl, "_blank", "noopener,noreferrer");
      }
    } catch (e) {
      setError(faucetErrorMessage(e));
    } finally {
      setSubmitting(false);
    }
  };

  const copyAddress = async () => {
    if (!result?.arcAddress) return;
    try {
      await copyTextToClipboard(result.arcAddress);
      setCopyState("copied");
      setTimeout(() => setCopyState("idle"), 1500);
    } catch {
      setCopyState("failed");
      setTimeout(() => setCopyState("idle"), 2600);
    }
  };

  if (result) {
    // Real mode: show the address + a faucet link so the user can paste and
    // complete the claim.
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
              Open test faucet
            </a>
            <button
              type="button"
              onClick={() => void copyAddress()}
              className="inline-flex items-center gap-1 text-[11px] font-mono text-text-lo hover:text-text-hi"
              title="Copy ARC address"
            >
              {copyState === "copied" ? (
                <Check className="w-3 h-3 text-accent-pnl" />
              ) : (
                <Copy className="w-3 h-3" />
              )}
              {copyState === "copied"
                ? "Copied"
                : copyState === "failed"
                  ? "Copy failed"
                  : `${result.arcAddress?.slice(0, 8)}…${result.arcAddress?.slice(-4)}`}
            </button>
          </div>
          {copyState === "failed" ? (
            <p className="max-w-full break-all text-[11px] text-risk font-mono">
              Copy failed. Use this address: {result.arcAddress}
            </p>
          ) : null}
          <p className="text-[11px] text-text-mut font-mono">
            Paste the address above into the faucet, select the available test
            network, and claim. Balance refreshes within ~30s.
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
        {submitting ? "Claiming…" : "Get test USDC"}
      </BrutalButton>
      <ProvenanceLine source="test faucet · 100 USDC/day" />
      {showFaucetLink ? (
        <a
          href="https://faucet.circle.com"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1 text-xs font-mono text-accent-agent hover:underline"
        >
          <ExternalLink className="w-3 h-3" />
          Open faucet
        </a>
      ) : null}
      {error && <span className="text-xs text-risk font-mono">{error}</span>}
    </div>
  );
}

function faucetErrorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  if (
    message.includes("faucet_daily_limit") ||
    message.includes("already requested today's test USDC")
  ) {
    return "You already requested today's test USDC. Open the faucet directly or try again tomorrow.";
  }
  return message.replace(/^[0-9]{3}: /, "");
}
