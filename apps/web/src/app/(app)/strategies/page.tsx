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
  label: "Ready",
  detail: "100% USDC. Approvals can execute on the routes that are live today.",
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
  (network) =>
    !EXECUTION_NETWORK_IDS.includes(
      network.blockchain as (typeof EXECUTION_NETWORK_IDS)[number],
    ),
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
    if (!tokenTargetable(symbol)) return;
    setSelectedId("custom");
    setName((current) => current || "Portfolio Target");
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

  async function saveTarget() {
    if (!authed) return;
    setCreating(true);
    setCreateError(null);
    try {
      const cleanName = name.trim() || selectedPreset.name;
      const routePreferences: RoutePreferences = {
        networks: [...EXECUTION_NETWORK_IDS],
        networkWatchlist: FUTURE_NETWORKS.map((network) => network.blockchain),
        tokens: targetTokens,
        watchlist: TRACK_ONLY_TOKEN_IDS.filter(
          (symbol) => !targetTokens.includes(symbol),
        ),
        updatedAt: new Date().toISOString(),
      };
      const goal: PortfolioGoal = {
        name: cleanName,
        objective:
          selectedPreset.riskTolerance === "conservative" ? "preserve" : "grow",
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
      setCreateError(e instanceof Error ? e.message : "Target save failed");
      setCreating(false);
    }
  }

  return (
    <div className="mx-auto max-w-[1120px]">
      <section className="rounded-sharp border-brutal border-border-default bg-surface shadow-brutal">
        <div className="grid gap-4 border-b border-border-default p-4 md:grid-cols-[minmax(0,1fr)_18rem] md:p-5">
          <div>
            <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
              Portfolio target
            </p>
            <h1 className="mt-2 text-2xl font-mono font-semibold text-text-hi">
              Choose what Aegis should aim for
            </h1>
            <p className="mt-2 max-w-2xl text-sm leading-relaxed text-text-lo">
              USDC can execute now. BTC, ETH, SOL, and EURC are tracked targets.
              USYC is coming soon and cannot be selected yet.
            </p>
          </div>
          <StatusPanel
            fullyExecutable={fullyExecutable}
            trackTotal={trackTotal}
          />
        </div>

        <div className="grid gap-5 p-4 md:p-5 xl:grid-cols-[minmax(0,1fr)_18rem]">
          <div className="space-y-5">
            <div>
              <label
                htmlFor="target-name"
                className="text-[10px] font-mono uppercase tracking-widest text-text-mut"
              >
                Target name
              </label>
              <input
                id="target-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                className="mt-2 w-full rounded-sharp border border-border-default bg-bg px-3 py-3 font-mono text-lg text-text-hi outline-none focus:border-border-hi"
              />
            </div>

            <div>
              <div className="flex flex-wrap items-center gap-2">
                <p className="mr-1 text-[10px] font-mono uppercase tracking-widest text-accent-agent">
                  Start from
                </p>
                {presets.map((preset) => (
                  <button
                    key={preset.id}
                    type="button"
                    onClick={() => selectPreset(preset)}
                    aria-pressed={selectedId === preset.id}
                    className={`rounded-sharp border px-3 py-2 text-left text-xs font-mono transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi ${
                      selectedId === preset.id
                        ? "border-accent-agent bg-accent-agent/10 text-text-hi"
                        : "border-border-default bg-bg text-text-lo hover:border-accent-agent/45 hover:text-text-hi"
                    }`}
                    title={preset.detail}
                  >
                    {preset.label}: {preset.name}
                  </button>
                ))}
                {isLoading && (
                  <span className="text-xs font-mono text-text-mut">
                    Loading recommendations…
                  </span>
                )}
              </div>
              {error && (
                <p className="mt-2 text-xs text-warn">
                  Recommendations are unavailable. Manual target editing still
                  works.
                </p>
              )}
            </div>

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
                  canAdd={trackTotal < 100}
                  onAdd={() => updateTrackWeight(token.id, 10)}
                  onRemove={() => updateTrackWeight(token.id, -10)}
                />
              ))}
              {COMING_SOON_TOKENS.map((token) => (
                <ComingSoonTokenRow
                  key={token.id}
                  symbol={token.symbol}
                  detail={token.detail}
                />
              ))}
            </div>
          </div>

          <aside className="space-y-3">
            <section className="rounded-sharp border border-accent-pnl/40 bg-accent-pnl/5 p-3">
              <p className="text-[10px] font-mono uppercase tracking-widest text-accent-pnl">
                Ready now
              </p>
              <p className="mt-2 font-mono text-sm text-text-hi">
                USDC on {executionNetworkLabels()}
              </p>
              <p className="mt-2 text-xs leading-relaxed text-text-lo">
                Saving a target never moves funds. Dashboard review is still
                required before execution.
              </p>
            </section>

            <section className="rounded-sharp border border-accent-agent/35 bg-accent-agent/5 p-3">
              <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
                Track only
              </p>
              <p className="mt-2 text-xs leading-relaxed text-text-lo">
                {TRACK_TOKENS.map((token) => token.symbol).join(", ")} can be
                saved as intent, but approval blocks until real routes are live.
              </p>
            </section>

            <section className="rounded-sharp border border-warn/45 bg-warn/5 p-3">
              <p className="text-[10px] font-mono uppercase tracking-widest text-warn">
                Coming soon
              </p>
              <p className="mt-2 text-xs leading-relaxed text-text-lo">
                {COMING_SOON_TOKENS.map((token) => token.symbol).join(", ")} is
                hidden from execution and target controls until support is
                verified.
              </p>
            </section>
          </aside>
        </div>

        <footer className="grid gap-3 border-t border-border-default p-4 md:grid-cols-[minmax(0,1fr)_auto] md:p-5">
          <div className="text-xs leading-relaxed text-text-lo">
            {hasPortfolio
              ? "Saving replaces your current target on the same portfolio."
              : "Saving creates your portfolio target."}{" "}
            No deployment happens until you approve a review on Dashboard.
            {createError && (
              <p role="alert" className="mt-2 font-mono text-risk">
                {createError}
              </p>
            )}
          </div>
          {authed ? (
            <button
              type="button"
              onClick={() => void saveTarget()}
              disabled={creating}
              className="inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-pnl px-5 font-semibold text-black transition-[box-shadow] hover:shadow-brutal-sm disabled:opacity-60"
            >
              {creating ? "Saving…" : "Save target"}
            </button>
          ) : (
            <Link
              href={authHref("/login", "/strategies")}
              className="inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-5 font-semibold text-black transition-[box-shadow] hover:shadow-brutal-sm"
            >
              Sign in to save
            </Link>
          )}
        </footer>
      </section>
    </div>
  );
}

function buildAllocation(
  trackWeights: Record<string, number>,
): Record<string, number> {
  const cleanTracks = Object.fromEntries(
    Object.entries(trackWeights).filter(
      ([symbol, pct]) => pct > 0 && tokenTargetable(symbol),
    ),
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
    label: "Agent",
    detail: strategyPreviewCopy(strategy),
    horizon: horizonFromMonths(strategy.minHorizonMonths),
    riskTolerance: riskFromBand(strategy.riskBand),
    targetAllocation: removeComingSoonTargets(strategy.targetAllocation),
  };
}

function strategyPreviewCopy(strategy: StrategyPublic) {
  if (strategy.name === "Conservative Treasury") {
    return "USDC reserve with EURC tracked for future FX support.";
  }
  if (strategy.name === "Operating Reserve") {
    return "USDC core plus EURC intent for future operating needs.";
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
  canAdd = false,
  onAdd,
  onRemove,
}: {
  symbol: string;
  percent: number;
  detail: string;
  ready?: boolean;
  canAdd?: boolean;
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
            disabled={!canAdd}
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

function ComingSoonTokenRow({
  symbol,
  detail,
}: {
  symbol: string;
  detail: string;
}) {
  return (
    <div className="grid gap-3 rounded-sharp border border-warn/35 bg-warn/5 p-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
      <div className="min-w-0">
        <p className="font-mono text-sm text-text-hi">{symbol}</p>
        <p className="mt-1 text-xs text-text-lo">{detail}</p>
      </div>
      <span className="justify-self-start rounded-sharp border border-warn/50 px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-warn sm:justify-self-end">
        Coming soon
      </span>
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
        {fullyExecutable ? "Executable" : "Needs route"}
      </p>
      <p className="mt-1 text-sm font-mono text-text-hi">
        {fullyExecutable ? "Can approve today" : `${trackTotal}% track-only`}
      </p>
      <p className="mt-1 text-xs leading-relaxed text-text-lo">
        {fullyExecutable
          ? "The dashboard review can execute on ready routes."
          : "Saving is allowed, but approval will block until missing routes are live."}
      </p>
    </div>
  );
}
