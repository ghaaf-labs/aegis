import type { Metadata } from "next";
import Link from "next/link";
import { StrategyCard } from "@/components/strategies/strategy-card";
import type { StrategyPublic } from "@/lib/api";

export const metadata: Metadata = {
  title: "Aegis · Strategy marketplace",
  description:
    "Browse curated stablecoin-native portfolio strategies. Adopt one with one tap; you still approve every rebalance.",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

async function fetchStrategies(): Promise<StrategyPublic[]> {
  try {
    const res = await fetch(`${API_BASE}/strategies`, {
      next: { revalidate: 60 },
    });
    if (!res.ok) return [];
    return (await res.json()) as StrategyPublic[];
  } catch {
    return [];
  }
}

export default async function PublicStrategiesPage() {
  const strategies = await fetchStrategies();

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

        {strategies.length === 0 ? (
          <section className="border-2 border-white/10 bg-[#141414] p-6 text-sm text-text-lo">
            No curated strategies yet. Run{" "}
            <code className="text-cyan-300">
              cargo run --bin seed_curated_strategies
            </code>{" "}
            against the dev DB to populate.
          </section>
        ) : (
          <section className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {strategies.map((s) => (
              <StrategyCard
                key={s.id}
                strategy={s}
                actionLabel="Sign up to adopt"
              />
            ))}
          </section>
        )}

        <footer className="text-xs text-text-mut font-mono">
          Want to adopt one?{" "}
          <Link href="/signup" className="text-accent-pnl hover:underline">
            Create a wallet
          </Link>
          .
        </footer>
      </div>
    </main>
  );
}
