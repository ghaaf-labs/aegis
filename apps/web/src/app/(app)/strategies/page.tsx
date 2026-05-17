"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { StrategyCard } from "@/components/strategies/strategy-card";
import { getToken, strategiesApi, type StrategyPublic } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";

// SM-3 / SM-4 — single /strategies route handles both authed and public
// visitors. Route groups (app)/(public) collapse to the same URL, so
// authentication is decided at render time by the presence of a JWT.
// Authed users get an "Adopt" button that calls strategiesApi.adopt();
// public visitors get a "Sign up to adopt" CTA routing to /signup.

export default function StrategiesPage() {
  const router = useRouter();
  const { data, error, isLoading } = useApiQuery<StrategyPublic[]>(
    "strategies.list",
    () => strategiesApi.list(),
  );
  const [adopting, setAdopting] = useState<string | null>(null);
  const [adoptError, setAdoptError] = useState<string | null>(null);

  const authed = typeof window !== "undefined" && getToken() !== null;

  const onAdopt = async (id: string) => {
    setAdopting(id);
    setAdoptError(null);
    try {
      const res = await strategiesApi.adopt(id);
      router.push(`/dashboard/${res.portfolioId}`);
    } catch (e) {
      setAdoptError(e instanceof Error ? e.message : "adopt failed");
      setAdopting(null);
    }
  };

  return (
    <main className="min-h-screen bg-bg text-text-default px-6 py-10">
      <div className="max-w-5xl mx-auto space-y-6">
        <header className="space-y-2">
          <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
            Strategy marketplace
          </p>
          <h1 className="text-3xl md:text-4xl font-mono font-semibold text-text-hi tracking-tight">
            Pick a starting allocation. Approve every move.
          </h1>
          <p className="text-sm text-text-lo max-w-2xl">
            Each curated strategy ships as a target allocation + risk band +
            horizon. Adopting one creates a new portfolio in your account seeded
            with that target; the agent never trades without your approval
            modal.
          </p>
        </header>

        {error && (
          <p className="text-xs font-mono text-rose-400">
            Failed to load strategies: {error.message}
          </p>
        )}
        {adoptError && (
          <p className="text-xs font-mono text-rose-400">
            Adopt failed: {adoptError}
          </p>
        )}

        {isLoading && !data ? (
          <p className="text-xs font-mono text-text-mut">Loading…</p>
        ) : (data ?? []).length === 0 ? (
          <section className="border-2 border-white/10 bg-[#141414] p-6 text-sm text-text-lo">
            No curated strategies yet. Run{" "}
            <code className="text-cyan-300">
              cargo run --bin seed_curated_strategies
            </code>{" "}
            against the dev DB to populate.
          </section>
        ) : (
          <section className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {(data ?? []).map((s) =>
              authed ? (
                <StrategyCard
                  key={s.id}
                  strategy={s}
                  actionLabel={adopting === s.id ? "Adopting…" : "Adopt"}
                  onAction={() => void onAdopt(s.id)}
                  disabled={adopting !== null}
                />
              ) : (
                <Link key={s.id} href="/signup" className="contents">
                  <StrategyCard strategy={s} actionLabel="Sign up to adopt" />
                </Link>
              ),
            )}
          </section>
        )}

        {!authed && (
          <footer className="text-xs text-text-mut font-mono">
            Already have an account?{" "}
            <Link
              href="/dashboard"
              className="text-accent-agent hover:underline"
            >
              Open dashboard
            </Link>
            . Want to adopt one?{" "}
            <Link href="/signup" className="text-accent-pnl hover:underline">
              Create a wallet
            </Link>
            .
          </footer>
        )}
      </div>
    </main>
  );
}
