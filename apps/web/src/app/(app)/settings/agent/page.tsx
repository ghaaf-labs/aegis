"use client";

import { useCallback, useEffect, useState } from "react";
import { BrutalButton, BrutalCard, BrutalCardBody } from "@aegis/ui";
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
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
          Agent control
        </h1>
        <p className="text-sm text-text-lo mt-1">
          One-toggle pause for every scheduled agent trigger — drift watcher,
          regime monitor, peg defense, scheduler tick. Manual rebalances and
          ad-hoc analyses are unaffected so you can still hand-drive when
          paused.
        </p>
      </div>

      <BrutalCard>
        <BrutalCardBody className="flex flex-col gap-4">
          <div className="flex items-baseline justify-between gap-4">
            <div>
              <p className="text-xs font-mono uppercase tracking-widest text-text-mut">
                Status
              </p>
              <p className="mt-1 text-xl font-mono font-semibold text-text-hi">
                {loading ? "Loading…" : isPaused ? "Paused" : "Active"}
              </p>
              {isPaused && pausedAt && (
                <p className="mt-1 text-xs text-text-lo">
                  Paused at {new Date(pausedAt).toLocaleString()}
                </p>
              )}
            </div>
            <BrutalButton
              variant={isPaused ? "agent" : "danger"}
              onClick={() => void toggle()}
              disabled={loading || busy}
            >
              {busy ? "Working…" : isPaused ? "Resume" : "Pause agent"}
            </BrutalButton>
          </div>

          <div className="pt-4 border-t border-border-default text-xs text-text-lo space-y-1">
            <p>
              Pausing is a soft stop. In-flight rebalances complete; only new
              scheduled triggers are gated.
            </p>
            <p>
              See{" "}
              <a className="text-accent-agent hover:underline" href="/policy">
                /policy
              </a>{" "}
              for the full outcome posture.
            </p>
          </div>

          {error && <p className="text-xs text-risk font-mono">{error}</p>}
        </BrutalCardBody>
      </BrutalCard>
    </div>
  );
}
