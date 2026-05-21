"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { StrategyCard } from "@/components/strategies/strategy-card";
import { getToken, strategiesApi, type StrategyPublic } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";
import { usePortfolioStore } from "@/stores/portfolio";

// SM-3 / SM-4 — single /strategies route handles both authed and public
// visitors. Authed users get an "Adopt" button; public visitors get a
// "Sign up to adopt" CTA. Auth check deferred to after hydration to avoid
// a "Sign up" flash for logged-in users.

export default function StrategiesPage() {
  const router = useRouter();
  const { data, error, isLoading } = useApiQuery<StrategyPublic[]>(
    "strategies.list",
    () => strategiesApi.list(),
  );
  const [adopting, setAdopting] = useState<string | null>(null);
  const [adoptError, setAdoptError] = useState<string | null>(null);
  const [authed, setAuthed] = useState(false);
  const hasPortfolio = usePortfolioStore((s) => s.portfolios.length > 0);
  useEffect(() => {
    setAuthed(getToken() !== null);
  }, []);

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
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
          Strategies
        </h1>
        <p className="text-sm text-text-lo mt-1">
          Pick a starting allocation. The agent never trades without your
          approval.
        </p>
      </div>

      {error && (
        <p className="text-xs font-mono text-risk">
          Failed to load strategies: {error.message}
        </p>
      )}
      {adoptError && (
        <p className="text-xs font-mono text-risk">
          Adopt failed: {adoptError}
        </p>
      )}
      {authed && hasPortfolio && (
        <p className="text-xs font-mono text-text-lo border-brutal border-border-default bg-raised px-3 py-2">
          Strategy adoption is available before you create a portfolio. Your
          current portfolio stays active;{" "}
          <Link href="/onboarding" className="text-accent-pnl hover:underline">
            open the rebuild wizard
          </Link>{" "}
          if you want new targets from scratch.
        </p>
      )}

      {isLoading && !data ? (
        <p className="text-xs font-mono text-text-mut">Loading…</p>
      ) : (data ?? []).length === 0 ? (
        <section className="border-brutal border-border-default bg-raised p-8 text-center space-y-2">
          <p className="text-sm font-mono text-text-lo">
            No strategies available yet — check back soon.
          </p>
          <p className="text-xs font-mono text-text-mut">
            Curated allocations are added regularly. You can always{" "}
            <Link
              href="/onboarding"
              className="text-accent-pnl hover:underline"
            >
              build a custom portfolio
            </Link>{" "}
            from scratch.
          </p>
        </section>
      ) : (
        <section className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {(data ?? []).map((s) =>
            authed ? (
              <StrategyCard
                key={s.id}
                strategy={s}
                actionLabel={
                  hasPortfolio
                    ? "Rebuild targets"
                    : adopting === s.id
                      ? "Adopting…"
                      : "Adopt"
                }
                actionHref={hasPortfolio ? "/onboarding" : undefined}
                onAction={hasPortfolio ? undefined : () => void onAdopt(s.id)}
                disabled={!hasPortfolio && adopting !== null}
                disabledReason={
                  hasPortfolio
                    ? "Creates a new portfolio in the wizard. Your current portfolio stays active until you switch."
                    : adopting !== null
                      ? "Finishing the current adoption request."
                      : undefined
                }
              />
            ) : (
              <StrategyCard
                key={s.id}
                strategy={s}
                actionLabel="Sign up to adopt"
                actionHref="/signup"
              />
            ),
          )}
        </section>
      )}

      {!authed && (
        <footer className="text-xs text-text-mut font-mono">
          Already have an account?{" "}
          <Link href="/dashboard" className="text-accent-agent hover:underline">
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
  );
}
