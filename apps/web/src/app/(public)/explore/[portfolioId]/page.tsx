import type { Metadata } from "next";
import { notFound } from "next/navigation";
import Link from "next/link";
import { Brain, ExternalLink, Shield } from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
  ModelBadge,
  ProvenanceLine,
} from "@aegis/ui";
import { DEMO_BUNDLES, DEMO_SLUGS } from "@/lib/explore-data";

/**
 * Public, SSR'd demo dashboard. No wallet, no auth, no SSE — fully indexable
 * by search engines so /explore drives top-of-funnel traffic.
 */

interface PageProps {
  params: Promise<{ portfolioId: string }>;
}

export async function generateStaticParams() {
  return DEMO_SLUGS.map((portfolioId) => ({ portfolioId }));
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { portfolioId } = await params;
  const bundle = DEMO_BUNDLES[portfolioId];
  if (!bundle) return { title: "Aegis · Explore" };
  return {
    title: `Aegis · ${bundle.portfolio.name} (demo)`,
    description: `Demo portfolio: ${bundle.portfolio.name}. See how the Aegis agent reasons about regime, allocation, and risk.`,
  };
}

export default async function ExploreDemoPage({ params }: PageProps) {
  const { portfolioId } = await params;
  const bundle = DEMO_BUNDLES[portfolioId];
  if (!bundle) notFound();

  const { portfolio, decisions } = bundle;
  const goal = portfolio.goal;

  return (
    <div className="min-h-screen bg-bg text-text-default">
      {/* Demo banner */}
      <div className="border-b-brutal border-accent-agent/40 bg-accent-agent/10">
        <div className="max-w-[1200px] mx-auto px-6 py-3 flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <Shield className="w-4 h-4 text-accent-agent" />
            <span className="text-xs font-mono text-text-hi">
              DEMO PORTFOLIO · {portfolio.name}
            </span>
            <span className="text-xs font-mono text-text-mut">
              Read-only · curated for /explore
            </span>
          </div>
          <Link
            href="/"
            className="inline-flex items-center gap-1 text-xs font-mono font-semibold text-accent-pnl hover:underline"
          >
            Sign up to build your own
            <ExternalLink className="w-3 h-3" />
          </Link>
        </div>
      </div>

      <div className="max-w-[1200px] mx-auto px-6 py-8 space-y-6">
        {/* Header */}
        <div className="flex items-baseline justify-between">
          <div>
            <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
              {portfolio.name}
            </h1>
            <ProvenanceLine source="curated demo · mock-data.ts" />
          </div>
          <div className="text-right">
            <p className="text-xs text-text-mut font-mono">PORTFOLIO VALUE</p>
            <p className="text-2xl font-mono font-semibold text-text-hi tabular-nums">
              ${portfolio.totalValueUsd.toLocaleString()}
            </p>
            <p
              className={`text-sm font-mono tabular-nums ${
                portfolio.totalPnlPct >= 0 ? "text-accent-pnl" : "text-risk"
              }`}
            >
              {portfolio.totalPnlPct >= 0 ? "+" : ""}
              {portfolio.totalPnlPct.toFixed(2)}% · $
              {portfolio.totalPnlUsd.toLocaleString()}
            </p>
          </div>
        </div>

        {/* Goal card */}
        {goal && (
          <BrutalCard>
            <BrutalCardHeader>
              <span className="text-sm font-mono font-semibold text-text-hi">
                Goal
              </span>
            </BrutalCardHeader>
            <BrutalCardBody>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm font-mono">
                <Stat label="Horizon" value={goal.horizon} />
                <Stat label="Risk tolerance" value={goal.riskTolerance} />
                <Stat
                  label="USYC sleeve"
                  value={goal.includeUsyc ? "on" : "off"}
                  highlight={goal.includeUsyc}
                />
                <Stat
                  label="EURC sleeve"
                  value={goal.includeEurc ? "on" : "off"}
                  highlight={goal.includeEurc}
                />
              </div>
              <div className="mt-4">
                <p className="text-xs text-text-mut font-mono mb-2">
                  TARGET ALLOCATION
                </p>
                <div className="flex flex-wrap gap-2">
                  {Object.entries(goal.targetAllocation).map(([sym, pct]) => (
                    <BrutalPill key={sym} tone="neutral">
                      {sym} {pct}%
                    </BrutalPill>
                  ))}
                </div>
              </div>
            </BrutalCardBody>
          </BrutalCard>
        )}

        {/* Decisions */}
        <BrutalCard>
          <BrutalCardHeader>
            <div className="flex items-center gap-2">
              <Brain className="w-4 h-4 text-accent-agent" />
              <span className="text-sm font-mono font-semibold text-text-hi">
                Agent reasoning
              </span>
            </div>
          </BrutalCardHeader>
          <BrutalCardBody>
            {decisions.map((d) => (
              <article
                key={d.id}
                className="border-b border-border-default last:border-b-0 pb-4 mb-4 last:mb-0 last:pb-0"
              >
                <div className="flex flex-wrap items-center gap-2 mb-2">
                  <BrutalPill
                    tone={
                      d.regime === "risk_on"
                        ? "pnl"
                        : d.regime === "risk_off"
                          ? "risk"
                          : "neutral"
                    }
                  >
                    {d.regime?.replace("_", "-").toUpperCase()}
                  </BrutalPill>
                  {d.modelSlug && <ModelBadge model={d.modelSlug} />}
                  <span className="text-xs font-mono text-text-mut">
                    confidence {Math.round((d.confidence ?? 0) * 100)}%
                  </span>
                </div>
                <p className="text-sm font-semibold text-text-hi mb-1">
                  {d.recommendation.summary}
                </p>
                <p className="text-xs text-text-lo leading-relaxed">
                  {d.reasoning}
                </p>
                {d.criticVerdict && (
                  <div className="mt-2 px-3 py-2 rounded-sharp border border-accent-pnl/30 bg-accent-pnl/5 text-[11px] font-mono text-text-default">
                    <span className="text-accent-pnl">Critic ✓</span>{" "}
                    <span className="text-text-mut">
                      ({Math.round(d.criticVerdict.confidence * 100)}%):
                    </span>{" "}
                    {d.criticVerdict.notes}
                  </div>
                )}
              </article>
            ))}
          </BrutalCardBody>
        </BrutalCard>

        <div className="text-center pt-4">
          <p className="text-xs text-text-mut font-mono mb-3">
            This is a curated demo. Real portfolios use Circle Wallets, Gateway
            unified USDC, USYC for yield, and StableFX for the EUR sleeve.
          </p>
          <Link
            href="/"
            className="inline-flex items-center gap-2 px-4 py-2 bg-accent-pnl text-black font-mono font-semibold rounded-sharp border-brutal border-black shadow-brutal-sm hover:shadow-brutal"
          >
            Build your own portfolio
            <ExternalLink className="w-3 h-3" />
          </Link>
        </div>
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  highlight,
}: {
  label: string;
  value: string;
  highlight?: boolean;
}) {
  return (
    <div>
      <p className="text-[10px] text-text-mut">{label.toUpperCase()}</p>
      <p
        className={`mt-0.5 ${highlight ? "text-accent-pnl" : "text-text-hi"} font-semibold`}
      >
        {value}
      </p>
    </div>
  );
}
