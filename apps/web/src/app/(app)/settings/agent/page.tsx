"use client";

import { useCallback, useEffect, useState } from "react";
import { userAgentApi } from "@/lib/api";

export const dynamic = "force-dynamic";

export default function AgentSettingsPage() {
  const [pausedAt, setPausedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    userAgentApi
      .status()
      .then((s) => {
        if (!cancelled) setPausedAt(s.pausedAt);
      })
      .catch((e) => {
        if (!cancelled)
          setError(e instanceof Error ? e.message : "Failed to load status");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const next = pausedAt
        ? await userAgentApi.resume()
        : await userAgentApi.pause();
      setPausedAt(next.pausedAt);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Toggle failed");
    } finally {
      setBusy(false);
    }
  }, [pausedAt]);

  const isPaused = pausedAt !== null;

  return (
    <div className="flex flex-col gap-6 max-w-3xl">
      <header>
        <h1 className="text-2xl font-bold tracking-tight">Agent control</h1>
        <p className="text-sm text-text-muted">
          One-toggle pause for every scheduled agent trigger — drift watcher,
          regime monitor, peg defense, scheduler tick. Manual rebalances and
          ad-hoc analyses are unaffected so you can still hand-drive when
          paused.
        </p>
      </header>

      <section className="border-2 border-white/10 bg-[#0F0F0F] p-6 flex flex-col gap-4">
        <div className="flex items-baseline justify-between gap-4">
          <div>
            <p className="text-xs font-mono uppercase tracking-widest text-text-mut">
              Status
            </p>
            <p className="mt-1 text-xl font-mono font-semibold">
              {loading ? "Loading…" : isPaused ? "Paused" : "Active"}
            </p>
            {isPaused && pausedAt && (
              <p className="mt-1 text-xs text-text-mut">
                Paused at {new Date(pausedAt).toLocaleString()}
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={toggle}
            disabled={loading || busy}
            aria-label={isPaused ? "Resume agent" : "Pause agent"}
            className={
              "px-4 py-2 text-sm font-mono uppercase tracking-widest border-2 transition-colors disabled:opacity-50 " +
              (isPaused
                ? "border-cyan-500/40 text-cyan-300 bg-cyan-500/10 hover:bg-cyan-500/20"
                : "border-rose-500/40 text-rose-300 bg-rose-500/10 hover:bg-rose-500/20")
            }
          >
            {busy ? "Working…" : isPaused ? "Resume" : "Pause agent"}
          </button>
        </div>

        <div className="pt-4 border-t border-white/5 text-xs text-text-mut space-y-1">
          <p>
            Pausing is a soft stop. In-flight rebalances complete; only new
            scheduled triggers are gated.
          </p>
          <p>
            See{" "}
            <a className="underline" href="/policy">
              /policy
            </a>{" "}
            for the full outcome posture.
          </p>
        </div>

        {error && <p className="text-xs text-rose-400 font-mono">{error}</p>}
      </section>
    </div>
  );
}
