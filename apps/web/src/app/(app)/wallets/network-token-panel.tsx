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
    state: "Ready",
    detail: "Wallet route ready",
  },
  {
    blockchain: "BASE-SEPOLIA",
    label: "Base Sepolia",
    state: "Ready",
    detail: "Wallet route ready",
  },
  {
    blockchain: "ETH-SEPOLIA",
    label: "Ethereum Sepolia",
    state: "Supported",
    detail: "Wallet route sync required",
  },
  {
    blockchain: "ARB-SEPOLIA",
    label: "Arbitrum Sepolia",
    state: "Supported",
    detail: "Wallet route sync required",
  },
  {
    blockchain: "AVAX-FUJI",
    label: "Avalanche Fuji",
    state: "Supported",
    detail: "Wallet route sync required",
  },
] as const;

const TOKENS = [
  {
    id: "USDC",
    symbol: "USDC",
    label: "Cash",
    state: "Core",
    detail: "Funding, transfer, and reserve route is live",
  },
  {
    id: "BTC",
    symbol: "BTC",
    label: "Market target",
    state: "Target",
    detail: "Pricing and swap planning available",
  },
  {
    id: "ETH",
    symbol: "ETH",
    label: "Market target",
    state: "Target",
    detail: "Pricing and swap planning available",
  },
  {
    id: "SOL",
    symbol: "SOL",
    label: "Market target",
    state: "Target",
    detail: "Pricing and swap planning available",
  },
  {
    id: "USYC",
    symbol: "USYC",
    label: "Yield target",
    state: "Target",
    detail: "Planner supports park and redeem routes",
  },
  {
    id: "EURC",
    symbol: "EURC",
    label: "FX target",
    state: "Target",
    detail: "Planner supports the StableFX sleeve",
  },
] as const;

const TARGET_TOKEN_IDS = TOKENS.map((token) => token.id);

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
  const watchedNetworkLabels = selectedLabels(
    NETWORKS,
    preferences.networkWatchlist ?? [],
    (network) => network.blockchain,
    (network) => network.label,
  );
  const syncNeededNetworkLabels = selectedLabels(
    NETWORKS,
    futureNetworkIds(liveNetworkIds),
    (network) => network.blockchain,
    (network) => network.label,
  );

  function chooseLiveRoutes() {
    commitPreferences(defaultPreferences(liveNetworkIds));
  }

  function chooseAgentSuggestion() {
    commitPreferences({
      networks: liveNetworkIds,
      networkWatchlist: futureNetworkIds(liveNetworkIds),
      tokens: TARGET_TOKEN_IDS,
      watchlist: [],
    });
  }

  function toggleNetwork(blockchain: string) {
    setPreferences((current) => {
      const executable = liveBlockchains.has(blockchain);
      const selected = new Set(
        executable ? current.networks : (current.networkWatchlist ?? []),
      );
      if (selected.has(blockchain) && executable && selected.size === 1) {
        return current;
      }
      if (selected.has(blockchain)) {
        selected.delete(blockchain);
      } else {
        selected.add(blockchain);
      }
      const next = executable
        ? {
            ...current,
            networks: orderByKnown([...selected], liveNetworkIds),
          }
        : {
            ...current,
            networkWatchlist: orderByKnown(
              [...selected],
              futureNetworkIds(liveNetworkIds),
            ),
          };
      persistPreferences(next);
      return next;
    });
  }

  function toggleToken(tokenId: string) {
    setPreferences((current) => {
      const selected = new Set(current.tokens);
      if (selected.has(tokenId) && selected.size === 1) {
        return current;
      }
      if (selected.has(tokenId)) {
        selected.delete(tokenId);
      } else {
        selected.add(tokenId);
      }
      const next = {
        ...current,
        tokens: orderByKnown([...selected], TARGET_TOKEN_IDS),
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
              Choose wallet routes the agent may use for account planning. Ready
              routes have an account address. Token execution still requires a
              live rail, price, and executor check.
            </p>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={chooseLiveRoutes}
                className="rounded-sharp border border-accent-pnl/50 bg-accent-pnl/10 px-3 py-2 text-xs font-mono text-accent-pnl transition-colors hover:bg-accent-pnl/15 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi"
              >
                Use ready routes
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
              Agent wallet scope
            </div>
            <dl className="mt-3 space-y-2 text-xs">
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Ready networks
                </dt>
                <dd className="text-text-hi">
                  {selectedNetworkLabels || "No live network selected"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Target tokens
                </dt>
                <dd className="text-text-hi">
                  {selectedTokenLabels || "No target token selected"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Watching
                </dt>
                <dd className="text-text-lo">
                  {watchedTokenLabels || "No extra token watchlist"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Wallet sync
                </dt>
                <dd className="text-text-lo">
                  {syncNeededNetworkLabels || "All supported routes ready"}
                  {watchedNetworkLabels
                    ? ` · queued: ${watchedNetworkLabels}`
                    : ""}
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
                const tracked = (preferences.networkWatchlist ?? []).includes(
                  network.blockchain,
                );
                return (
                  <button
                    type="button"
                    key={network.blockchain}
                    onClick={() => toggleNetwork(network.blockchain)}
                    aria-pressed={live ? selected : tracked}
                    className={`rounded-sharp border p-3 text-left transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi disabled:cursor-not-allowed ${
                      live
                        ? selected
                          ? "border-accent-pnl bg-accent-pnl/10"
                          : "border-accent-pnl/40 bg-accent-pnl/5 hover:bg-accent-pnl/10"
                        : tracked
                          ? "border-accent-agent/60 bg-accent-agent/5"
                          : "border-border-default bg-raised hover:border-accent-agent/40 hover:bg-accent-agent/5"
                    }`}
                  >
                    <div className="flex flex-wrap items-start justify-between gap-2">
                      <div className="min-w-0">
                        <p className="text-sm font-mono leading-snug text-text-hi">
                          {network.label}
                        </p>
                        <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                          {live
                            ? selected
                              ? "Included in wallet scope"
                              : network.detail
                            : tracked
                              ? "Queued for wallet sync"
                              : network.detail}
                        </p>
                      </div>
                      <StatusPill
                        tone={live ? "live" : tracked ? "agent" : "muted"}
                      >
                        {live
                          ? selected
                            ? "Ready"
                            : network.state
                          : tracked
                            ? "Sync"
                            : "Needs sync"}
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
                    preferences.tokens.includes(token.id)
                      ? "border-accent-pnl/45 bg-accent-pnl/5"
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
                    <StatusPill tone="live">
                      {preferences.tokens.includes(token.id)
                        ? "Included"
                        : token.state}
                    </StatusPill>
                    <button
                      type="button"
                      onClick={() => toggleToken(token.id)}
                      aria-pressed={preferences.tokens.includes(token.id)}
                      className="rounded-sharp border border-border-default bg-bg px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-text-lo transition-colors hover:border-accent-pnl hover:text-accent-pnl focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi"
                    >
                      {preferences.tokens.includes(token.id)
                        ? "Remove"
                        : "Include"}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>

        <div className="flex items-start gap-2 rounded-sharp border border-border-default bg-bg p-3 text-[11px] leading-relaxed text-text-lo">
          <LockKeyhole className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warn" />
          <p className="min-w-0">
            Selection is an agent instruction, not a bypass. A token action
            becomes executable only after the wallet network, transfer rail,
            pricing, and executor tests are all live. Watched items shape
            analysis only.{" "}
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
    networkWatchlist: [],
    tokens: liveNetworkIds.length > 0 ? TARGET_TOKEN_IDS : [],
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
  const requestedTokens = normalizeTokenIds(
    preferences.tokens
      ? [...preferences.tokens, ...(preferences.watchlist ?? [])]
      : undefined,
  );
  const tokens = orderByKnown(
    requestedTokens.filter((id) => knownTokens.has(id)),
    TARGET_TOKEN_IDS,
  );
  const futureNetworks = futureNetworkIds(liveNetworkIds);
  const promotedNetworks = [
    ...(preferences.networks ?? []),
    ...(preferences.networkWatchlist ?? []).filter((id) => live.has(id)),
  ];
  const networks = orderByKnown(
    promotedNetworks.filter((id) => live.has(id)),
    liveNetworkIds,
  );
  return {
    networks: networks.length > 0 ? networks : fallback.networks,
    networkWatchlist: orderByKnown(
      (preferences.networkWatchlist ?? []).filter((id) =>
        futureNetworks.includes(id),
      ),
      futureNetworks,
    ),
    tokens: tokens.length > 0 ? tokens : fallback.tokens,
    watchlist: orderByKnown(
      (preferences.watchlist ?? []).filter(
        (id) => knownTokens.has(id) && !tokens.includes(id),
      ),
      TOKENS.map((token) => token.id),
    ),
    updatedAt: preferences.updatedAt,
  };
}

function normalizeTokenIds(tokens: string[] | undefined): string[] {
  const selected = new Set(tokens ?? TARGET_TOKEN_IDS);
  if (selected.delete("BTC_ETH_SOL")) {
    selected.add("BTC");
    selected.add("ETH");
    selected.add("SOL");
  }
  return [...selected];
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

function futureNetworkIds(liveNetworkIds: string[]): string[] {
  const live = new Set(liveNetworkIds);
  return NETWORKS.map((network) => network.blockchain).filter(
    (id) => !live.has(id),
  );
}

function StatusPill({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "live" | "warn" | "muted" | "agent";
}) {
  return (
    <span
      className={`shrink-0 whitespace-nowrap rounded-sharp border px-2 py-1 text-[10px] font-mono uppercase tracking-wider ${
        tone === "live"
          ? "border-accent-pnl/50 bg-accent-pnl/10 text-accent-pnl"
          : tone === "agent"
            ? "border-accent-agent/50 bg-accent-agent/10 text-accent-agent"
            : tone === "warn"
              ? "border-warn/50 bg-warn/10 text-warn"
              : "border-border-default bg-bg text-text-mut"
      }`}
    >
      {children}
    </span>
  );
}
