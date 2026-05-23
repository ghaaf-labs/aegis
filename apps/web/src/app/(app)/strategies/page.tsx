"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { portfolioApi, strategiesApi, type StrategyPublic } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";
import {
  COMING_SOON_TOKEN_IDS,
  EXECUTION_NETWORK_IDS,
  NETWORK_ROUTE_OPTIONS,
  TOKEN_ROUTE_OPTIONS,
  TRACK_ONLY_TOKEN_IDS,
  executionReady,
  tokenRouteOption,
  tokenTargetable,
} from "@/lib/route-capabilities";
import { useApiQuery } from "@/lib/use-api-query";
import { usePortfolioStore } from "@/stores/portfolio";
import type {
  AssetSymbol,
  GoalHorizon,
  PortfolioGoal,
  RiskTolerance,
  RoutePreferences,
} from "@/types";

type BuilderPreset = {
  id: string;
  name: string;
  label: string;
  detail: string;
  horizon: GoalHorizon;
  riskTolerance: RiskTolerance;
  targetAllocation: Record<string, number>;
};

const READY_PRESET: BuilderPreset = {
  id: "ready-usdc",
  name: "USDC Reserve",
  label: "Ready today",
  detail: "Keeps the first approval fully executable: USDC on Arc/Base only.",
  horizon: "1y",
  riskTolerance: "conservative",
  targetAllocation: { USDC: 100 },
};

const TRACK_TOKENS = TOKEN_ROUTE_OPTIONS.filter(
  (token) => !token.executable && token.targetable,
);
const COMING_SOON_TOKENS = TOKEN_ROUTE_OPTIONS.filter(
  (token) => !token.targetable,
);
const FUTURE_NETWORKS = NETWORK_ROUTE_OPTIONS.filter(
  (network) => !executionReady(network.blockchain),
);

export default function StrategiesPage() {
  const router = useRouter();
  const { data, error, isLoading } = useApiQuery<StrategyPublic[]>(
    "strategies.list",
    () => strategiesApi.list(),
  );
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const hasPortfolio = usePortfolioStore((s) => s.portfolios.length > 0);
  const addPortfolio = usePortfolioStore((s) => s.addPortfolio);
  const authed = sessionResolved && sessionActive;

  const presets = useMemo(
    () => [READY_PRESET, ...(data ?? []).map(strategyToPreset)],
    [data],
  );
  const [selectedId, setSelectedId] = useState(READY_PRESET.id);
  const [name, setName] = useState(READY_PRESET.name);
  const [trackWeights, setTrackWeights] = useState<Record<string, number>>({});
  const [networks, setNetworks] = useState<string[]>([
    ...EXECUTION_NETWORK_IDS,
  ]);
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  const selectedPreset =
    presets.find((preset) => preset.id === selectedId) ?? READY_PRESET;
  const targetAllocation = useMemo(
    () => buildAllocation(trackWeights),
    [trackWeights],
  );
  const trackTotal = 100 - (targetAllocation.USDC ?? 0);
  const fullyExecutable = trackTotal === 0;
  const targetTokens = Object.entries(targetAllocation)
    .filter(([, pct]) => pct > 0)
    .map(([symbol]) => symbol);

  function selectPreset(preset: BuilderPreset) {
    setSelectedId(preset.id);
    setName(preset.name);
    setTrackWeights(extractTrackWeights(preset.targetAllocation));
    setCreateError(null);
  }

  function updateTrackWeight(symbol: string, delta: number) {
    setSelectedId("custom");
    setName((current) => current || "Custom Target");
    setTrackWeights((current) => {
      const next = { ...current };
      const currentValue = next[symbol] ?? 0;
      const usedByOthers = Object.entries(next)
        .filter(([key]) => key !== symbol)
        .reduce((sum, [, value]) => sum + value, 0);
      const maxForToken = Math.max(0, 100 - usedByOthers);
      const updated = Math.max(0, Math.min(maxForToken, currentValue + delta));
      if (updated === 0) {
        delete next[symbol];
      } else {
        next[symbol] = updated;
      }
      return next;
    });
  }

  function toggleNetwork(blockchain: string) {
    setNetworks((current) => {
      if (current.includes(blockchain)) {
        return current.length === 1
          ? current
          : current.filter((id) => id !== blockchain);
      }
      return [...current, blockchain].filter(
        (id, index, all) => all.indexOf(id) === index,
      );
    });
  }

  async function createTarget() {
    if (!authed) return;
    setCreating(true);
    setCreateError(null);
    try {
      const cleanName = name.trim() || selectedPreset.name;
      const routePreferences: RoutePreferences = {
        networks,
        networkWatchlist: FUTURE_NETWORKS.map((network) => network.blockchain),
        tokens: targetTokens,
        watchlist: TRACK_ONLY_TOKEN_IDS.filter(
          (symbol) => !targetTokens.includes(symbol),
        ),
        updatedAt: new Date().toISOString(),
      };
      const goal: PortfolioGoal = {
        name: cleanName,
        horizon: selectedPreset.horizon,
        riskTolerance: selectedPreset.riskTolerance,
        targetAllocation: targetAllocation as Partial<
          Record<AssetSymbol, number>
        >,
        includeUsyc: false,
        includeEurc: (targetAllocation.EURC ?? 0) > 0,
        routePreferences,
        createdAt: new Date().toISOString(),
      };
      const allocations = Object.entries(targetAllocation)
        .filter(([, pct]) => pct > 0)
        .map(([symbol, pct]) => ({
          symbol,
          quantity: 0,
          targetWeight: pct,
        }));
      const portfolio = await portfolioApi.create({
        name: cleanName,
        allocations,
        goal,
      });
      addPortfolio({
        ...portfolio,
        allocations: allocations.map((allocation) => ({
          assetId: allocation.symbol,
          symbol: allocation.symbol,
          quantity: 0,
          targetWeight: allocation.targetWeight,
          currentWeight: 0,
          valueUsd: 0,
        })),
      });
      router.push(`/dashboard/${portfolio.id}`);
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : "target creation failed");
      setCreating(false);
    }
  }

  return (
    <div className="mx-auto max-w-[1180px] space-y-5">
      <header className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div>
          <h1 className="text-2xl font-mono font-semibold tracking-tight text-text-hi">
            Build a Portfolio
          </h1>
          <p className="mt-1 max-w-2xl text-sm leading-relaxed text-text-lo">
            Start with one target. USDC can execute now. BTC, ETH, SOL, and EURC
            can be tracked, but they pause approval until their routes are live.
            USYC is coming soon and cannot be selected yet.
          </p>
        </div>
        <section className="rounded-sharp border border-accent-pnl/40 bg-accent-pnl/5 p-3">
          <p className="text-[10px] font-mono uppercase tracking-widest text-accent-pnl">
            Ready route
          </p>
          <p className="mt-1 text-sm font-mono text-text-hi">
            USDC on {executionNetworkLabels()}
          </p>
          <p className="mt-1 text-xs text-text-lo">
            Every approval still opens a review before money moves.
          </p>
        </section>
      </header>

      {error && (
        <section className="border-brutal border-risk/40 bg-risk/5 p-4 text-sm text-text-lo">
          <p className="font-semibold text-text-hi">
            Couldn&apos;t load recommendations
          </p>
          <p className="mt-1 text-xs leading-relaxed">
            You can still use the USDC-only target or adjust the mix manually.
          </p>
        </section>
      )}

      <section className="grid gap-4 lg:grid-cols-[18rem_minmax(0,1fr)]">
        <aside className="space-y-3 rounded-sharp border border-border-default bg-surface p-3">
          <div>
            <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
              1. Choose a starting point
            </p>
            <p className="mt-1 text-xs text-text-lo">
              Pick one. You can tune tokens on the right.
            </p>
          </div>
          <div className="grid gap-2">
            {presets.map((preset) => (
              <button
                key={preset.id}
                type="button"
                onClick={() => selectPreset(preset)}
                aria-pressed={selectedId === preset.id}
                className={`rounded-sharp border p-3 text-left transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi ${
                  selectedId === preset.id
                    ? "border-accent-agent bg-accent-agent/10"
                    : "border-border-default bg-bg hover:border-accent-agent/45"
                }`}
              >
                <span className="block text-[10px] font-mono uppercase tracking-widest text-text-mut">
                  {preset.label}
                </span>
                <span className="mt-1 block text-sm font-mono font-semibold text-text-hi">
                  {preset.name}
                </span>
                <span className="mt-1 block text-xs leading-relaxed text-text-lo">
                  {preset.detail}
                </span>
              </button>
            ))}
            {isLoading && (
              <p className="px-1 text-xs font-mono text-text-mut">
                Loading recommendations…
              </p>
            )}
          </div>
        </aside>

        <section className="rounded-sharp border-brutal border-border-default bg-surface shadow-brutal">
          <div className="grid gap-4 border-b border-border-default p-4 lg:grid-cols-[minmax(0,1fr)_16rem]">
            <div>
              <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
                2. Tune the target
              </p>
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                className="mt-2 w-full rounded-sharp border border-border-default bg-bg px-3 py-2 font-mono text-lg text-text-hi outline-none focus:border-border-hi"
                aria-label="Portfolio target name"
              />
              <p className="mt-2 text-xs text-text-lo">
                USDC fills the remainder automatically, so the target always
                totals 100%.
              </p>
            </div>
            <StatusPanel
              fullyExecutable={fullyExecutable}
              trackTotal={trackTotal}
            />
          </div>

          <div className="grid gap-4 p-4 xl:grid-cols-[minmax(0,1fr)_18rem]">
            <div className="space-y-3">
              <TokenRow
                symbol="USDC"
                percent={targetAllocation.USDC ?? 0}
                detail="Executable cash reserve"
                ready
              />
              {TRACK_TOKENS.map((token) => (
                <TokenRow
                  key={token.id}
                  symbol={token.symbol}
                  percent={trackWeights[token.id] ?? 0}
                  detail={token.label}
                  onAdd={() => updateTrackWeight(token.id, 10)}
                  onRemove={() => updateTrackWeight(token.id, -10)}
                />
              ))}
            </div>

            <aside className="space-y-3">
              <section className="rounded-sharp border border-border-default bg-bg p-3">
                <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
                  Execution chains
                </p>
                <div className="mt-3 grid gap-2">
                  {EXECUTION_NETWORK_IDS.map((blockchain) => (
                    <button
                      key={blockchain}
                      type="button"
                      onClick={() => toggleNetwork(blockchain)}
                      aria-pressed={networks.includes(blockchain)}
                      className={`rounded-sharp border px-3 py-2 text-left text-xs font-mono transition-colors ${
                        networks.includes(blockchain)
                          ? "border-accent-pnl bg-accent-pnl/10 text-accent-pnl"
                          : "border-border-default bg-raised text-text-lo"
                      }`}
                    >
                      {networkLabel(blockchain)} · Ready
                    </button>
                  ))}
                </div>
              </section>

              <section className="rounded-sharp border border-accent-agent/35 bg-accent-agent/5 p-3">
                <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
                  Track only
                </p>
                <p className="mt-2 text-xs leading-relaxed text-text-lo">
                  {FUTURE_NETWORKS.map((network) => network.label).join(", ")}
                  {" and "}
                  {TRACK_TOKENS.map((token) => token.symbol).join(", ")} stay
                  visible for planning only.
                </p>
              </section>

              <section className="rounded-sharp border border-border-default bg-bg p-3">
                <p className="text-[10px] font-mono uppercase tracking-widest text-text-mut">
                  Coming soon
                </p>
                <p className="mt-2 text-xs leading-relaxed text-text-lo">
                  {COMING_SOON_TOKENS.map((token) => token.symbol).join(", ")}{" "}
                  is not selectable until its live route is verified.
                </p>
              </section>
            </aside>
          </div>

          <footer className="grid gap-3 border-t border-border-default p-4 md:grid-cols-[minmax(0,1fr)_auto]">
            <div className="text-xs leading-relaxed text-text-lo">
              {hasPortfolio
                ? "This replaces your current target on the same portfolio."
                : "This creates your portfolio target."}{" "}
              No deployment happens until you approve a review on the dashboard.
              {createError && (
                <p role="alert" className="mt-2 font-mono text-risk">
                  {createError}
                </p>
              )}
            </div>
            {authed ? (
              <button
                type="button"
                onClick={() => void createTarget()}
                disabled={creating}
                className="inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-pnl px-5 font-semibold text-black transition-[box-shadow] hover:shadow-brutal-sm disabled:opacity-60"
              >
                {creating
                  ? "Saving…"
                  : fullyExecutable
                    ? "Save executable target"
                    : "Save tracked target"}
              </button>
            ) : (
              <Link
                href={authHref("/login", "/strategies")}
                className="inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-5 font-semibold text-black transition-[box-shadow] hover:shadow-brutal-sm"
              >
                Sign in to create
              </Link>
            )}
          </footer>
        </section>
      </section>
    </div>
  );
}

function buildAllocation(
  trackWeights: Record<string, number>,
): Record<string, number> {
  const cleanTracks = Object.fromEntries(
    Object.entries(trackWeights).filter(([, pct]) => pct > 0),
  );
  const trackTotal = Object.values(cleanTracks).reduce(
    (sum, pct) => sum + pct,
    0,
  );
  return {
    USDC: Math.max(0, 100 - trackTotal),
    ...cleanTracks,
  };
}

function extractTrackWeights(allocation: Record<string, number>) {
  return Object.fromEntries(
    Object.entries(allocation).filter(
      ([symbol, pct]) =>
        symbol !== "USDC" && pct > 0 && tokenTargetable(symbol),
    ),
  );
}

function strategyToPreset(strategy: StrategyPublic): BuilderPreset {
  return {
    id: strategy.id,
    name: strategy.name,
    label: "Recommended",
    detail: strategyPreviewCopy(strategy),
    horizon: horizonFromMonths(strategy.minHorizonMonths),
    riskTolerance: riskFromBand(strategy.riskBand),
    targetAllocation: removeComingSoonTargets(strategy.targetAllocation),
  };
}

function strategyPreviewCopy(strategy: StrategyPublic) {
  if (strategy.name === "Conservative Treasury") {
    return "Mostly USDC, with EURC tracked until its route is live.";
  }
  if (strategy.name === "Operating Reserve") {
    return "USDC core plus EURC target for future operating needs.";
  }
  if (strategy.name === "Balanced") {
    return "USDC core plus tracked BTC and ETH exposure.";
  }
  return strategy.description.split(".")[0] + ".";
}

function removeComingSoonTargets(
  allocation: Record<string, number>,
): Record<string, number> {
  const next = { ...allocation };
  const movedToUsdc = COMING_SOON_TOKEN_IDS.reduce((sum, symbol) => {
    const value = next[symbol] ?? 0;
    delete next[symbol];
    return sum + value;
  }, 0);
  return { ...next, USDC: (next.USDC ?? 0) + movedToUsdc };
}

function horizonFromMonths(months: number): GoalHorizon {
  if (months <= 12) return "1y";
  if (months <= 36) return "3y";
  return "5y";
}

function riskFromBand(band: StrategyPublic["riskBand"]): RiskTolerance {
  if (band === "low") return "conservative";
  if (band === "high") return "aggressive";
  return "moderate";
}

function executionNetworkLabels() {
  return NETWORK_ROUTE_OPTIONS.filter((network) =>
    EXECUTION_NETWORK_IDS.includes(
      network.blockchain as (typeof EXECUTION_NETWORK_IDS)[number],
    ),
  )
    .map((network) => network.label)
    .join(" + ");
}

function networkLabel(blockchain: string) {
  return (
    NETWORK_ROUTE_OPTIONS.find((network) => network.blockchain === blockchain)
      ?.label ?? blockchain
  );
}

function authHref(path: "/login", next: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

function TokenRow({
  symbol,
  percent,
  detail,
  ready = false,
  onAdd,
  onRemove,
}: {
  symbol: string;
  percent: number;
  detail: string;
  ready?: boolean;
  onAdd?: () => void;
  onRemove?: () => void;
}) {
  const route = tokenRouteOption(symbol);
  const active = percent > 0;
  return (
    <div
      className={`grid gap-3 rounded-sharp border p-3 sm:grid-cols-[minmax(0,1fr)_8rem_auto] sm:items-center ${
        ready
          ? "border-accent-pnl/45 bg-accent-pnl/5"
          : active
            ? "border-accent-agent/45 bg-accent-agent/5"
            : "border-border-default bg-bg"
      }`}
    >
      <div className="min-w-0">
        <p className="font-mono text-sm text-text-hi">{symbol}</p>
        <p className="mt-1 text-xs text-text-lo">
          {detail}
          {route && !route.executable ? " · Track only" : ""}
        </p>
      </div>
      <div>
        <div className="h-2 border border-border-default bg-bg">
          <div
            className={
              ready ? "h-full bg-accent-pnl" : "h-full bg-accent-agent"
            }
            style={{ width: `${percent}%` }}
          />
        </div>
        <p className="mt-1 text-right font-mono text-sm text-text-hi tabular-nums">
          {percent}%
        </p>
      </div>
      {ready ? (
        <span className="justify-self-start rounded-sharp border border-accent-pnl/50 px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-accent-pnl sm:justify-self-end">
          Ready
        </span>
      ) : (
        <div className="flex gap-2 sm:justify-end">
          <button
            type="button"
            onClick={onRemove}
            disabled={!active}
            className="h-9 w-10 rounded-sharp border border-border-default bg-raised font-mono text-text-hi disabled:opacity-40"
            aria-label={`Reduce ${symbol}`}
          >
            -
          </button>
          <button
            type="button"
            onClick={onAdd}
            disabled={percent >= 100}
            className="h-9 w-10 rounded-sharp border border-accent-agent/50 bg-accent-agent/10 font-mono text-accent-agent disabled:opacity-40"
            aria-label={`Add ${symbol}`}
          >
            +
          </button>
        </div>
      )}
    </div>
  );
}

function StatusPanel({
  fullyExecutable,
  trackTotal,
}: {
  fullyExecutable: boolean;
  trackTotal: number;
}) {
  return (
    <div
      className={`rounded-sharp border p-3 ${
        fullyExecutable
          ? "border-accent-pnl/45 bg-accent-pnl/5"
          : "border-warn/50 bg-warn/5"
      }`}
    >
      <p
        className={`text-[10px] font-mono uppercase tracking-widest ${
          fullyExecutable ? "text-accent-pnl" : "text-warn"
        }`}
      >
        {fullyExecutable ? "Fully executable" : "Tracked target"}
      </p>
      <p className="mt-1 text-sm font-mono text-text-hi">
        {fullyExecutable
          ? "Approval can run today"
          : `${trackTotal}% track-only`}
      </p>
      <p className="mt-1 text-xs leading-relaxed text-text-lo">
        {fullyExecutable
          ? "The review can approve USDC movement on ready routes."
          : "Aegis will create the target, then block approval until missing routes are removed or connected."}
      </p>
    </div>
  );
}
