import type { Metadata } from "next";
import Link from "next/link";
import { ProvenanceLine } from "@aegis/ui";
import { BacktestChart, type Sample } from "@/components/regime/backtest-chart";

export const metadata: Metadata = {
  title: "Aegis · Regime classifier — 5y backtest",
  description:
    "Replay of the Aegis regime classifier across the last several years of price history. Trust signal, not marketing.",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

interface SamplesResponse {
  evalRunId: string | null;
  modelSlug: string | null;
  samples: Sample[];
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

async function fetchSamples(): Promise<SamplesResponse | null> {
  try {
    const res = await fetch(
      `${API_BASE}/about/regime/backtest/samples?limit=1200`,
      { next: { revalidate: 60 } },
    );
    if (!res.ok) return null;
    return (await res.json()) as SamplesResponse;
  } catch {
    return null;
  }
}

export default async function RegimeBacktestPage() {
  const data = await fetchSamples();

  return (
    <main className="min-h-screen bg-bg text-text-default px-6 py-10">
      <div className="max-w-4xl mx-auto space-y-6">
        <header className="space-y-2">
          <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
            Regime classifier · backtest
          </p>
          <h1 className="text-3xl font-mono font-semibold text-text-hi tracking-tight">
            How the classifier called the last few years
          </h1>
          <p className="text-sm text-text-lo max-w-2xl">
            Each tick is one out-of-sample prediction the regime classifier made
            when replayed across historical price + cross-asset correlation
            features. Cyan = RISK-ON, neutral = NEUTRAL, rose = RISK-OFF.
            Realized regime is the dotted line.
          </p>
        </header>

        {data && data.samples.length > 0 ? (
          <section className="border-2 border-white/10 bg-[#141414] p-4 space-y-3">
            <BacktestChart samples={data.samples} />
            <ProvenanceLine
              source={`backtest replay · model ${data.modelSlug ?? "unknown"}`}
              freshness={`run ${(data.evalRunId ?? "—").slice(0, 8)}`}
            />
          </section>
        ) : (
          <section className="border-2 border-white/10 bg-[#141414] p-6 text-sm text-text-lo">
            No backtest has been persisted yet. Run{" "}
            <code className="text-accent-agent">
              POST /admin/regime/backtest
            </code>{" "}
            (auth required) to populate; this page re-fetches once samples land.
          </section>
        )}

        <footer className="text-xs text-text-mut font-mono">
          Headline metrics (accuracy, precision, F1) live on the{" "}
          <Link
            href="/about/regime"
            className="text-accent-agent hover:underline"
          >
            model card
          </Link>
          . Source data lives in `model_evaluation_samples`; queries are in
          `apps/api/src/modules/risk_engine/regime_backtest.rs`.
        </footer>
      </div>
    </main>
  );
}
