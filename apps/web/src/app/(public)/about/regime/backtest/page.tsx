import type { Metadata } from "next";
import Link from "next/link";
import { ProvenanceLine } from "@aegis/ui";
import { BacktestChart, type Sample } from "@/components/regime/backtest-chart";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata({
  title: "Regime Classifier Backtest Evidence — Aegis",
  description:
    "Backtest evidence for the Aegis market-regime classifier. Out-of-sample predictions replayed across historical price data.",
  path: "/about/regime/backtest",
});

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
            Regime classifier — backtest evidence
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
          <section className="border-2 border-white/10 bg-[#141414] p-8 text-center space-y-3">
            <p className="text-sm font-mono font-semibold text-text-hi">
              Backtest evidence isn&apos;t published yet.
            </p>
            <p className="text-xs text-text-lo max-w-md mx-auto">
              Evaluation samples will appear here once the first backtest run is
              complete. Check back after the classifier has processed enough
              live decisions.
            </p>
          </section>
        )}

        <footer className="text-xs text-text-mut font-mono">
          Headline metrics (accuracy, precision, F1) live on the{" "}
          <Link
            href="/about/regime"
            className="inline-flex min-h-9 items-center text-accent-agent hover:underline"
          >
            model card
          </Link>
          .
        </footer>
      </div>
    </main>
  );
}
