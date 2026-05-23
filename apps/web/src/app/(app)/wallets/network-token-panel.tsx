"use client";

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Bot, Cpu, LockKeyhole, ShieldCheck } from "lucide-react";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";
import type { RoutePreferences } from "@/types";

interface WalletNetworkRoute {
  blockchain: string;
  address: string;
}

interface NetworkTokenPanelProps {
  networks: WalletNetworkRoute[];
  initialPreferences?: RoutePreferences | null;
  persistenceLabel?: string;
  onPreferencesChange?: (preferences: RoutePreferences) => void;
}

const NETWORKS = [
  {
    blockchain: "ARC-TESTNET",
    label: "Arc testnet",
    state: "Live",
    detail: "USDC cash route",
  },
  {
    blockchain: "BASE-SEPOLIA",
    label: "Base Sepolia",
    state: "Live",
    detail: "USDC cash route",
  },
  {
    blockchain: "ETH-SEPOLIA",
    label: "Ethereum Sepolia",
    state: "Next",
    detail: "Circle-supported; Aegis route not opened yet",
  },
  {
    blockchain: "ARB-SEPOLIA",
    label: "Arbitrum Sepolia",
    state: "Next",
    detail: "Circle-supported; Aegis route not opened yet",
  },
  {
    blockchain: "AVAX-FUJI",
    label: "Avalanche Fuji",
    state: "Next",
    detail: "Circle-supported; Aegis route not opened yet",
  },
] as const;

const TOKENS = [
  {
    id: "USDC",
    symbol: "USDC",
    label: "Cash",
    state: "Executable",
    detail: "Funding, bridge, and reserve route is live",
    mode: "execute",
  },
  {
    id: "BTC_ETH_SOL",
    symbol: "BTC / ETH / SOL",
    label: "Market targets",
    state: "Watch only",
    detail: "Track intent until live swap routes are ready",
    mode: "track",
  },
  {
    id: "USYC",
    symbol: "USYC",
    label: "Yield",
    state: "Watch only",
    detail: "Track intent until real yield execution is enabled",
    mode: "track",
  },
  {
    id: "EURC",
    symbol: "EURC",
    label: "FX sleeve",
    state: "Watch only",
    detail: "Track intent until StableFX execution is enabled",
    mode: "track",
  },
] as const;

const PREF_KEY = "aegis.wallet.route-preferences.v1";

export function NetworkTokenPanel({
  networks,
  initialPreferences,
  persistenceLabel = "Saved on this device",
  onPreferencesChange,
}: NetworkTokenPanelProps) {
  const liveBlockchains = useMemo(
    () => new Set(networks.map((network) => network.blockchain)),
    [networks],
  );
  const liveNetworkIds = useMemo(
    () =>
      NETWORKS.filter((network) => liveBlockchains.has(network.blockchain)).map(
        (network) => network.blockchain,
      ),
    [liveBlockchains],
  );
  const [preferences, setPreferences] = useState<RoutePreferences>(() =>
    defaultPreferences(liveNetworkIds),
  );

  useEffect(() => {
    setPreferences(
      sanitizePreferences(
        initialPreferences ?? loadPreferences(liveNetworkIds),
        liveNetworkIds,
      ),
    );
  }, [initialPreferences, liveNetworkIds]);

  const selectedNetworkLabels = selectedLabels(
    NETWORKS,
    preferences.networks,
    (network) => network.blockchain,
    (network) => network.label,
  );
  const selectedTokenLabels = selectedLabels(
    TOKENS,
    preferences.tokens,
    (token) => token.id,
    (token) => token.symbol,
  );
  const watchedTokenLabels = selectedLabels(
    TOKENS,
    preferences.watchlist,
    (token) => token.id,
    (token) => token.symbol,
  );

  function chooseLiveRoutes() {
    commitPreferences(defaultPreferences(liveNetworkIds));
  }

  function chooseAgentSuggestion() {
    commitPreferences({
      networks: liveNetworkIds,
      tokens: ["USDC"],
      watchlist: ["BTC_ETH_SOL", "USYC", "EURC"],
    });
  }

  function toggleNetwork(blockchain: string) {
    if (!liveBlockchains.has(blockchain)) {
      return;
    }
    setPreferences((current) => {
      const selected = new Set(current.networks);
      if (selected.has(blockchain)) {
        if (selected.size === 1) {
          return current;
        }
        selected.delete(blockchain);
      } else {
        selected.add(blockchain);
      }
      const next = {
        ...current,
        networks: orderByKnown([...selected], liveNetworkIds),
      };
      persistPreferences(next);
      return next;
    });
  }

  function toggleWatchlist(tokenId: string) {
    setPreferences((current) => {
      const selected = new Set(current.watchlist);
      if (selected.has(tokenId)) {
        selected.delete(tokenId);
      } else {
        selected.add(tokenId);
      }
      const next = {
        ...current,
        watchlist: orderByKnown(
          [...selected],
          TOKENS.map((token) => token.id),
        ),
      };
      persistPreferences(next);
      return next;
    });
  }

  function commitPreferences(next: RoutePreferences) {
    const sanitized = sanitizePreferences(next, liveNetworkIds);
    setPreferences(sanitized);
    persistPreferences(sanitized);
  }

  function persistPreferences(next: RoutePreferences) {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(PREF_KEY, JSON.stringify(next));
    }
    onPreferencesChange?.(next);
  }

  return (
    <BrutalCard>
      <BrutalCardHeader className="gap-3">
        <span className="flex min-w-0 items-center gap-2 text-sm font-mono text-text-hi">
          <Cpu className="h-4 w-4 shrink-0 text-accent-agent" />
          Networks & tokens
        </span>
        <span className="shrink-0 text-[10px] font-mono uppercase tracking-wider text-text-mut">
          Agent scope
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-4">
        <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_20rem]">
          <div className="space-y-3">
            <p className="max-w-3xl text-xs leading-relaxed text-text-lo">
              Choose what the agent is allowed to use now. Today approvals can
              execute USDC across Arc testnet and Base Sepolia. Other
              Circle-supported routes are saved as intent until the wallet,
              pricing, and executor path are live.
            </p>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={chooseLiveRoutes}
                className="rounded-sharp border border-accent-pnl/50 bg-accent-pnl/10 px-3 py-2 text-xs font-mono text-accent-pnl transition-colors hover:bg-accent-pnl/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi"
              >
                Use live routes
              </button>
              <button
                type="button"
                onClick={chooseAgentSuggestion}
                className="inline-flex items-center gap-2 rounded-sharp border border-accent-agent/50 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent transition-colors hover:bg-accent-agent/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi"
              >
                <Bot className="h-3.5 w-3.5" />
                Agent suggestion
              </button>
            </div>
          </div>

          <section
            aria-label="Route preference summary"
            className="rounded-sharp border border-accent-agent/40 bg-accent-agent/5 p-3"
          >
            <div className="text-[10px] font-mono uppercase tracking-wider text-accent-agent">
              Agent can execute now
            </div>
            <dl className="mt-3 space-y-2 text-xs">
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Networks
                </dt>
                <dd className="text-text-hi">
                  {selectedNetworkLabels || "No live network selected"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Tokens
                </dt>
                <dd className="text-text-hi">
                  {selectedTokenLabels || "No executable token selected"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Watching
                </dt>
                <dd className="text-text-lo">
                  {watchedTokenLabels || "No blocked token tracked"}
                </dd>
              </div>
            </dl>
          </section>
        </div>

        <div className="grid gap-3 lg:grid-cols-[1.1fr_0.9fr]">
          <section aria-label="Network routes" className="space-y-2">
            <div className="flex items-center gap-2 text-[10px] font-mono uppercase tracking-wider text-text-mut">
              <ShieldCheck className="h-3.5 w-3.5 text-accent-pnl" />
              Network routes
            </div>
            <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {NETWORKS.map((network) => {
                const live = liveBlockchains.has(network.blockchain);
                const selected = preferences.networks.includes(
                  network.blockchain,
                );
                return (
                  <button
                    type="button"
                    key={network.blockchain}
                    disabled={!live}
                    onClick={() => toggleNetwork(network.blockchain)}
                    aria-pressed={selected}
                    className={`rounded-sharp border p-3 text-left transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi disabled:cursor-not-allowed ${
                      live
                        ? selected
                          ? "border-accent-pnl bg-accent-pnl/10"
                          : "border-accent-pnl/40 bg-accent-pnl/5 hover:bg-accent-pnl/10"
                        : "border-border-default bg-raised opacity-75"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-mono text-text-hi">
                          {network.label}
                        </p>
                        <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                          {live
                            ? selected
                              ? "Allowed for agent execution"
                              : network.detail
                            : network.detail}
                        </p>
                      </div>
                      <StatusPill tone={live ? "live" : "muted"}>
                        {live
                          ? selected
                            ? "Selected"
                            : network.state
                          : "Needs route"}
                      </StatusPill>
                    </div>
                  </button>
                );
              })}
            </div>
          </section>

          <section aria-label="Token routes" className="space-y-2">
            <div className="text-[10px] font-mono uppercase tracking-wider text-text-mut">
              Token routes
            </div>
            <div className="grid gap-2">
              {TOKENS.map((token) => (
                <div
                  key={token.symbol}
                  className={`grid grid-cols-[minmax(0,1fr)_auto] gap-3 rounded-sharp border p-3 ${
                    token.mode === "execute"
                      ? "border-accent-pnl/45 bg-accent-pnl/5"
                      : preferences.watchlist.includes(token.id)
                        ? "border-warn/60 bg-warn/5"
                        : "border-border-default bg-raised"
                  }`}
                >
                  <div className="min-w-0">
                    <p className="text-sm font-mono text-text-hi">
                      {token.symbol}
                    </p>
                    <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                      {token.label} · {token.detail}
                    </p>
                  </div>
                  <div className="flex flex-col items-end gap-2">
                    <StatusPill
                      tone={token.mode === "execute" ? "live" : "warn"}
                    >
                      {token.state}
                    </StatusPill>
                    {token.mode === "track" ? (
                      <button
                        type="button"
                        onClick={() => toggleWatchlist(token.id)}
                        aria-pressed={preferences.watchlist.includes(token.id)}
                        className="rounded-sharp border border-border-default bg-bg px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-text-lo transition-colors hover:border-warn hover:text-warn focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi"
                      >
                        {preferences.watchlist.includes(token.id)
                          ? "Tracked"
                          : "Track"}
                      </button>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-[10px] font-mono uppercase tracking-wider text-accent-pnl">
                        <ShieldCheck className="h-3 w-3" />
                        Allowed
                      </span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>

        <div className="flex items-start gap-2 rounded-sharp border border-border-default bg-bg p-3 text-[11px] leading-relaxed text-text-lo">
          <LockKeyhole className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warn" />
          <p className="min-w-0">
            Selection is an agent instruction, not a bypass. A route becomes
            executable only after the wallet network, Circle rail, pricing, and
            executor tests are all live.{" "}
            <span className="font-mono uppercase tracking-wider text-text-mut">
              {persistenceLabel}
            </span>
          </p>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function defaultPreferences(liveNetworkIds: string[]): RoutePreferences {
  return {
    networks: liveNetworkIds,
    tokens: liveNetworkIds.length > 0 ? ["USDC"] : [],
    watchlist: [],
  };
}

function loadPreferences(liveNetworkIds: string[]): RoutePreferences {
  if (typeof window === "undefined") {
    return defaultPreferences(liveNetworkIds);
  }

  const fallback = defaultPreferences(liveNetworkIds);
  try {
    const stored = JSON.parse(
      window.localStorage.getItem(PREF_KEY) ?? "null",
    ) as Partial<RoutePreferences> | null;
    if (!stored) {
      return fallback;
    }
    return sanitizePreferences(stored, liveNetworkIds);
  } catch {
    return fallback;
  }
}

function sanitizePreferences(
  preferences: Partial<RoutePreferences>,
  liveNetworkIds: string[],
): RoutePreferences {
  const fallback = defaultPreferences(liveNetworkIds);
  const live = new Set(liveNetworkIds);
  const knownTokens = new Set<string>(TOKENS.map((token) => token.id));
  const networks = orderByKnown(
    (preferences.networks ?? []).filter((id) => live.has(id)),
    liveNetworkIds,
  );
  return {
    networks: networks.length > 0 ? networks : fallback.networks,
    tokens: (preferences.tokens ?? ["USDC"]).filter((id) => id === "USDC"),
    watchlist: orderByKnown(
      (preferences.watchlist ?? []).filter(
        (id) => knownTokens.has(id) && id !== "USDC",
      ),
      TOKENS.map((token) => token.id),
    ),
    updatedAt: preferences.updatedAt,
  };
}

function selectedLabels<TItem>(
  items: readonly TItem[],
  selectedIds: string[],
  idOf: (item: TItem) => string,
  labelOf: (item: TItem) => string,
): string {
  const selected = new Set(selectedIds);
  return items
    .filter((item) => selected.has(idOf(item)))
    .map(labelOf)
    .join(", ");
}

function orderByKnown(ids: string[], order: readonly string[]): string[] {
  const selected = new Set(ids);
  return order.filter((id) => selected.has(id));
}

function StatusPill({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "live" | "warn" | "muted";
}) {
  return (
    <span
      className={`rounded-sharp border px-2 py-1 text-[10px] font-mono uppercase tracking-wider ${
        tone === "live"
          ? "border-accent-pnl/50 bg-accent-pnl/10 text-accent-pnl"
          : tone === "warn"
            ? "border-warn/50 bg-warn/10 text-warn"
            : "border-border-default bg-bg text-text-mut"
      }`}
    >
      {children}
    </span>
  );
}
