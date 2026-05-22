"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { StrategyCard } from "@/components/strategies/strategy-card";
import { portfolioApi, strategiesApi, type StrategyPublic } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";
import { useApiQuery } from "@/lib/use-api-query";
import { usePortfolioStore } from "@/stores/portfolio";

// SM-3 / SM-4 — single /strategies route handles both authed and public
// visitors. Authed users get an "Adopt" button; public visitors continue with
// email. Session state comes from SessionBootstrap so the public CTA never
// flashes for signed-in users.

export default function StrategiesPage() {
  const router = useRouter();
  const { data, error, isLoading } = useApiQuery<StrategyPublic[]>(
    "strategies.list",
    () => strategiesApi.list(),
  );
  const [adopting, setAdopting] = useState<string | null>(null);
  const [adoptError, setAdoptError] = useState<string | null>(null);
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const hasPortfolio = usePortfolioStore((s) => s.portfolios.length > 0);
  const addPortfolio = usePortfolioStore((s) => s.addPortfolio);
  const authed = sessionResolved && sessionActive;

  const onAdopt = async (id: string) => {
    setAdopting(id);
    setAdoptError(null);
    try {
      const res = await strategiesApi.adopt(id);
      const portfolio = await portfolioApi.get(res.portfolioId);
      addPortfolio(portfolio);
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
        <section className="grid gap-4 border-brutal border-border-default bg-raised p-4 md:grid-cols-[1fr_280px]">
          <div className="space-y-2">
            <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
              Existing portfolio detected
            </p>
            <p className="text-xs font-mono leading-relaxed text-text-lo">
              Adopting a strategy now creates a separate portfolio from the
              selected card. Your current portfolio stays untouched, and no USDC
              moves until you open the new dashboard and approve a deploy or
              rebalance plan.
            </p>
            <p className="text-xs font-mono text-text-mut">
              Want a completely custom target instead?{" "}
              <Link
                href="/onboarding"
                className="inline-flex min-h-9 items-center text-accent-agent hover:underline"
              >
                Open the build-from-scratch wizard
              </Link>
              .
            </p>
          </div>
          <StrategyAdoptionSvg />
        </section>
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
              className="inline-flex min-h-9 items-center text-accent-pnl hover:underline"
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
                  adopting === s.id
                    ? "Creating portfolio…"
                    : hasPortfolio
                      ? "Adopt as new portfolio"
                      : "Adopt strategy"
                }
                onAction={() => void onAdopt(s.id)}
                disabled={adopting !== null}
                disabledReason={
                  adopting !== null && adopting !== s.id
                    ? "Finishing the current adoption request."
                    : hasPortfolio
                      ? "Creates a separate portfolio from this strategy. Review deployment before any money moves."
                      : "Creates a portfolio target from this strategy. You still approve every deploy."
                }
              />
            ) : (
              <StrategyCard
                key={s.id}
                strategy={s}
                actionLabel="Continue"
                actionHref={authHref("/login", "/strategies")}
                actionTone="agent"
                disabledReason="Use one email code. Aegis signs you in or creates the account, then returns here."
              />
            ),
          )}
        </section>
      )}

      {!sessionResolved ? null : !authed ? (
        <footer className="text-xs text-text-mut font-mono">
          Ready to adopt a strategy?{" "}
          <Link
            href={authHref("/login", "/strategies")}
            className="inline-flex min-h-9 items-center rounded-sharp text-accent-agent hover:underline"
          >
            Continue with email
          </Link>
          .
        </footer>
      ) : null}
    </div>
  );
}

function authHref(path: "/login", next: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

function StrategyAdoptionSvg() {
  const steps = [
    { x: 30, label: "Pick", value: "Strategy" },
    { x: 118, label: "Create", value: "Portfolio" },
    { x: 214, label: "Approve", value: "USDC" },
  ];

  return (
    <svg
      viewBox="0 0 280 88"
      role="img"
      aria-label="Strategy adoption creates a portfolio before any USDC deployment"
      className="h-auto w-full border border-border-default bg-bg"
    >
      <defs>
        <pattern
          id="strategy-adoption-grid"
          width="14"
          height="14"
          patternUnits="userSpaceOnUse"
        >
          <path d="M14 0H0V14" fill="none" stroke="#242424" strokeWidth="1" />
        </pattern>
      </defs>
      <rect width="280" height="88" fill="url(#strategy-adoption-grid)" />
      <path
        d="M70 48H102M160 48H196"
        fill="none"
        stroke="#00E0FF"
        strokeWidth="3"
        strokeLinecap="square"
      />
      {steps.map((step, index) => (
        <g key={step.label} transform={`translate(${step.x} 18)`}>
          <rect
            width="56"
            height="60"
            fill="#141414"
            stroke={index === 2 ? "#00FF88" : "#00E0FF"}
            strokeWidth="2"
          />
          <text
            x="28"
            y="22"
            textAnchor="middle"
            fontFamily="monospace"
            fontSize="9"
            fill="#8A8A8A"
          >
            {step.label}
          </text>
          <text
            x="28"
            y="40"
            textAnchor="middle"
            fontFamily="monospace"
            fontSize="9"
            fontWeight="700"
            fill="#FFFFFF"
          >
            {step.value}
          </text>
        </g>
      ))}
    </svg>
  );
}
