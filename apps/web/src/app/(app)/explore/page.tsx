import Link from "next/link";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { DEMO_BUNDLES } from "@/lib/explore-data";

export const metadata = {
  title: "Aegis · Explore demo portfolios",
  description:
    "Three curated Aegis portfolios across different risk profiles and regimes — see how the agent reasons before signing up.",
};

export default function ExploreIndex() {
  const bundles = Object.values(DEMO_BUNDLES);
  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
          Explore demo portfolios
        </h1>
        <p className="text-sm text-text-lo mt-1 max-w-2xl">
          Three curated portfolios for different risk profiles. No wallet, no
          signup — just see how the agent reasons.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {bundles.map(({ portfolio }) => (
          <Link
            key={portfolio.id}
            href={`/explore/${portfolio.id}`}
            className="block group"
          >
            <BrutalCard className="cursor-pointer transition-all group-hover:border-accent-agent/60 group-hover:translate-x-[-2px] group-hover:translate-y-[-2px] group-hover:shadow-brutal">
              <BrutalCardHeader>
                <div className="flex items-center justify-between w-full">
                  <span className="font-mono font-semibold text-text-hi">
                    {portfolio.name}
                  </span>
                  <BrutalPill tone="agent">
                    {portfolio.goal?.horizon}
                  </BrutalPill>
                </div>
              </BrutalCardHeader>
              <BrutalCardBody>
                <p className="text-2xl font-mono font-semibold text-text-hi tabular-nums">
                  ${portfolio.totalValueUsd.toLocaleString()}
                </p>
                <p
                  className={`text-sm font-mono mt-1 ${
                    portfolio.totalPnlPct >= 0 ? "text-accent-pnl" : "text-risk"
                  }`}
                >
                  {portfolio.totalPnlPct >= 0 ? "+" : ""}
                  {portfolio.totalPnlPct.toFixed(1)}% all-time
                </p>
                <p className="text-xs text-text-mut mt-3">
                  Risk: {portfolio.goal?.riskTolerance}
                </p>
                <p className="text-[11px] text-accent-agent/70 group-hover:text-accent-agent font-mono mt-3">
                  Open the agent diary →
                </p>
              </BrutalCardBody>
            </BrutalCard>
          </Link>
        ))}
      </div>

      <div className="pt-2">
        <Link
          href="/signup"
          className="inline-flex items-center gap-2 px-4 py-2 bg-accent-pnl text-black font-mono font-semibold rounded-sharp border-brutal border-black shadow-brutal-sm hover:shadow-brutal"
        >
          Sign up to build your own
        </Link>
      </div>
    </div>
  );
}
