"use client";

import { useState } from "react";

import { cn } from "@/lib/utils";

export interface DiaryVisibilityToggleProps {
  initialPublic: boolean;
  onChange: (next: boolean) => Promise<void>;
  walletAddress?: string;
}

/**
 * Single-control opt-in for public agent diary visibility.
 * Default is `false` (private). Flipping on exposes the portfolio's decisions
 * at `/diary/[wallet]`; flipping off hides them immediately on next request.
 */
export function DiaryVisibilityToggle({
  initialPublic,
  onChange,
  walletAddress,
}: DiaryVisibilityToggleProps) {
  const [isPublic, setIsPublic] = useState(initialPublic);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggle = async () => {
    const next = !isPublic;
    setBusy(true);
    setError(null);
    try {
      await onChange(next);
      setIsPublic(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : "update failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      data-testid="diary-toggle"
      className="border-2 border-white/10 bg-[#141414] p-4 space-y-3"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-sm font-semibold text-text-hi">
              Public agent diary
            </p>
            <span
              className={cn(
                "border px-1.5 py-0.5 text-[10px] font-mono uppercase",
                isPublic
                  ? "border-accent-agent/40 bg-accent-agent/10 text-accent-agent"
                  : "border-white/10 bg-white/5 text-text-mut",
              )}
            >
              {isPublic ? "Public" : "Private"}
            </span>
          </div>
          <p className="text-xs text-text-lo mt-1">
            When on, anyone can see every recommendation the agent emits for
            this portfolio at{" "}
            <code className="text-accent-agent font-mono">
              /diary/
              {walletAddress ? walletAddress.slice(0, 10) + "…" : "your wallet"}
            </code>
            . Off by default.
          </p>
        </div>
        <button
          type="button"
          aria-label={
            isPublic ? "Make agent diary private" : "Make agent diary public"
          }
          aria-pressed={isPublic}
          onClick={toggle}
          disabled={busy}
          className={cn(
            "shrink-0 w-12 h-6 border-2",
            isPublic
              ? "bg-cyan-400 border-cyan-200"
              : "bg-gray-700 border-gray-500",
            busy && "opacity-50 cursor-not-allowed",
          )}
        >
          <span
            className={cn(
              "block w-4 h-4 bg-black transition-transform",
              isPublic ? "translate-x-6" : "translate-x-0",
            )}
          />
        </button>
      </div>
      {error && (
        <p className="text-xs text-risk font-mono" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
