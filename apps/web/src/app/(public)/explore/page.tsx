import Link from "next/link";
import { Shield } from "lucide-react";
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
    <div className="min-h-screen bg-bg text-text-default">
      <div className="max-w-[1100px] mx-auto px-6 py-12 space-y-8">
        <header className="flex items-center gap-3">
          <Shield className="w-5 h-5 text-accent-pnl" />
          <h1 className="text-2xl font-mono font-semibold text-text-hi">
            Explore Aegis
          </h1>
        </header>
        <p className="text-sm text-text-lo max-w-2xl">
          Three curated portfolios for different risk profiles. No wallet, no
          signup — just see how the agent reasons.
        </p>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {bundles.map(({ portfolio }) => (
            <Link key={portfolio.id} href={`/explore/${portfolio.id}`}>
              <BrutalCard className="cursor-pointer">
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
                      portfolio.totalPnlPct >= 0
                        ? "text-accent-pnl"
                        : "text-risk"
                    }`}
                  >
                    {portfolio.totalPnlPct >= 0 ? "+" : ""}
                    {portfolio.totalPnlPct.toFixed(1)}% all-time
                  </p>
                  <p className="text-xs text-text-mut mt-3">
                    Risk: {portfolio.goal?.riskTolerance}
                  </p>
                </BrutalCardBody>
              </BrutalCard>
            </Link>
          ))}
        </div>

        <div className="pt-4">
          <Link
            href="/"
            className="inline-flex items-center gap-2 px-4 py-2 bg-accent-pnl text-black font-mono font-semibold rounded-sharp border-brutal border-black shadow-brutal-sm hover:shadow-brutal"
          >
            Sign up to build your own
          </Link>
        </div>
      </div>
    </div>
  );
}
