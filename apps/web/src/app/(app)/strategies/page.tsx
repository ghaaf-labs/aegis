"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { StrategyCard } from "@/components/strategies/strategy-card";
import { strategiesApi, type StrategyPublic } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";

export default function AuthedStrategiesPage() {
  const router = useRouter();
  const { data, error, isLoading } = useApiQuery<StrategyPublic[]>(
    "strategies.list",
    () => strategiesApi.list(),
  );
  const [adopting, setAdopting] = useState<string | null>(null);
  const [adoptError, setAdoptError] = useState<string | null>(null);

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
      <header className="space-y-2">
        <h1 className="text-2xl md:text-3xl font-mono font-semibold text-text-hi tracking-tight">
          Strategy marketplace
        </h1>
        <p className="text-sm text-text-lo max-w-2xl">
          Adopt a curated allocation. A new portfolio lands in your account with
          the strategy&apos;s target; you fund it and approve every rebalance.
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
        <p className="text-xs font-mono text-text-mut">
          No curated strategies yet.
        </p>
      ) : (
        <section className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {(data ?? []).map((s) => (
            <StrategyCard
              key={s.id}
              strategy={s}
              actionLabel={adopting === s.id ? "Adopting…" : "Adopt"}
              onAction={() => void onAdopt(s.id)}
              disabled={adopting !== null}
            />
          ))}
        </section>
      )}
    </div>
  );
}
