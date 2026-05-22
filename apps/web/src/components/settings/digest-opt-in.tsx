"use client";

import { useEffect, useState } from "react";
import { Mail } from "lucide-react";
import { digestApi, analyticsApi } from "@/lib/api";

interface Props {
  /** Pre-fill the email field. Pulled from the user's wallet profile. */
  defaultEmail?: string;
}

/**
 * Subscribe to the agent's daily digest email. Re-rendered on success with a
 * confirmation pill so the user sees the opt-in landed.
 */
export function DigestOptIn({ defaultEmail = "" }: Props) {
  const [email, setEmail] = useState(defaultEmail);
  const [busy, setBusy] = useState(false);
  const [subscribed, setSubscribed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmedEmail = email.trim();
  const canSubscribe = trimmedEmail.includes("@");

  useEffect(() => {
    if (!defaultEmail) return;
    setEmail((current) => (current.trim() ? current : defaultEmail));
  }, [defaultEmail]);

  const handle = async () => {
    setBusy(true);
    setError(null);
    try {
      await digestApi.subscribe(trimmedEmail);
      setSubscribed(true);
      void analyticsApi.track("digest.subscribed", { method: "settings" });
    } catch (e) {
      setError(e instanceof Error ? e.message : "subscribe failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="border-2 border-white/10 bg-[#141414] p-4 space-y-3">
      <div className="flex items-center gap-2">
        <Mail className="w-3.5 h-3.5 text-accent-agent" />
        <span className="text-xs font-semibold text-text-hi">
          Daily agent digest
        </span>
        {subscribed && (
          <span className="text-[10px] font-mono uppercase tracking-wider text-accent-pnl border border-accent-pnl/30 bg-accent-pnl/5 px-1.5 py-0.5">
            subscribed
          </span>
        )}
      </div>
      <p className="text-[11px] text-text-lo leading-relaxed">
        One short email per day: regime read, what the agent did (or held off
        on), and what to watch. Unsubscribe with a single click — the link is in
        every email.
      </p>
      {!subscribed && (
        <>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@example.com"
            autoComplete="email"
            className="min-h-10 w-full rounded-sharp border-brutal border-border-default bg-bg px-3 py-2 font-mono text-sm text-text-hi outline-none focus:border-border-hi"
          />
          <div className="flex items-center justify-between gap-2">
            <button
              type="button"
              onClick={() => void handle()}
              disabled={!canSubscribe || busy}
              className="inline-flex min-h-9 items-center justify-center border-2 border-accent-agent bg-accent-agent px-3 text-xs font-semibold text-black hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {busy ? "Subscribing…" : "Subscribe"}
            </button>
            {!canSubscribe && !error && (
              <p className="text-[11px] text-text-mut font-mono">
                Enter an email address to enable.
              </p>
            )}
            {error && (
              <p className="text-[11px] text-risk font-mono" role="alert">
                {error}
              </p>
            )}
          </div>
        </>
      )}
    </div>
  );
}
