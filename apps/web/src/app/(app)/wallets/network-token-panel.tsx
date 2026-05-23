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
    detail: "Wallet address and rebalance rail are ready",
  },
  {
    blockchain: "BASE-SEPOLIA",
    label: "Base Sepolia",
    detail: "Wallet address and rebalance rail are ready",
  },
  {
    blockchain: "ETH-SEPOLIA",
    label: "Ethereum Sepolia",
    detail: "Wallet address can be synced; rebalance rail comes later",
  },
  {
    blockchain: "ARB-SEPOLIA",
    label: "Arbitrum Sepolia",
    detail: "Wallet address can be synced; rebalance rail comes later",
  },
  {
    blockchain: "AVAX-FUJI",
    label: "Avalanche Fuji",
    detail: "Wallet address can be synced; rebalance rail comes later",
  },
] as const;

// Plain-language route states match the backend registry vocabulary:
// Ready, Track only, Needs route, Needs quote, Needs address. A token is only
// `executable` when its route is Ready.
const TOKENS = [
  {
    id: "USDC",
    symbol: "USDC",
    label: "Cash",
    state: "Ready",
    detail: "Reserve, funding, and transfer route is ready",
    executable: true,
  },
  {
    id: "BTC",
    symbol: "BTC",
    label: "Market target",
    state: "Track only",
    detail: "Price tracking is ready; swap execution is not connected yet",
    executable: false,
  },
  {
    id: "ETH",
    symbol: "ETH",
    label: "Market target",
    state: "Track only",
    detail: "Price tracking is ready; swap execution is not connected yet",
    executable: false,
  },
  {
    id: "SOL",
    symbol: "SOL",
    label: "Market target",
    state: "Track only",
    detail: "Price tracking is ready; swap execution is not connected yet",
    executable: false,
  },
  {
    id: "USYC",
    symbol: "USYC",
    label: "Yield target",
    state: "Track only",
    detail:
      "Yield parking is turned off — the USYC Teller on Arc is allowlist-gated, so USYC is tracked only for now",
    executable: false,
  },
  {
    id: "EURC",
    symbol: "EURC",
    label: "FX target",
    state: "Track only",
    detail: "FX tracking is ready; Arc StableFX execution is KYB-gated",
    executable: false,
  },
] as const;

const TARGET_TOKEN_IDS = TOKENS.map((token) => token.id);
const EXECUTABLE_TOKEN_IDS = TOKENS.filter((token) => token.executable).map(
  (token) => token.id,
);
const TRACK_ONLY_TOKEN_IDS = TOKENS.filter((token) => !token.executable).map(
  (token) => token.id,
);
const EXECUTION_NETWORK_IDS = ["ARC-TESTNET", "BASE-SEPOLIA"];

const PREF_KEY = "aegis.wallet.route-preferences.v2";

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
  const executionNetworkLabels = selectedLabels(
    NETWORKS,
    liveNetworkIds.filter((id) => EXECUTION_NETWORK_IDS.includes(id)),
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
      tokens: liveNetworkIds.length > 0 ? EXECUTABLE_TOKEN_IDS : [],
      watchlist: TRACK_ONLY_TOKEN_IDS,
    });
  }

  function toggleNetwork(blockchain: string) {
    const executable = liveBlockchains.has(blockchain);
    const selected = new Set(
      executable ? preferences.networks : (preferences.networkWatchlist ?? []),
    );
    if (selected.has(blockchain) && executable && selected.size === 1) {
      return;
    }
    if (selected.has(blockchain)) {
      selected.delete(blockchain);
    } else {
      selected.add(blockchain);
    }
    commitPreferences(
      executable
        ? {
            ...preferences,
            networks: orderByKnown([...selected], liveNetworkIds),
          }
        : {
            ...preferences,
            networkWatchlist: orderByKnown(
              [...selected],
              futureNetworkIds(liveNetworkIds),
            ),
          },
    );
  }

  function toggleToken(tokenId: string) {
    const targetTokens = new Set(preferences.tokens);
    const watchlist = new Set(preferences.watchlist);
    if (targetTokens.has(tokenId) && targetTokens.size === 1) {
      return;
    }
    if (targetTokens.has(tokenId)) {
      targetTokens.delete(tokenId);
      if (!tokenExecutable(tokenId)) {
        watchlist.add(tokenId);
      }
    } else {
      targetTokens.add(tokenId);
      watchlist.delete(tokenId);
    }
    commitPreferences({
      ...preferences,
      tokens: orderByKnown([...targetTokens], TARGET_TOKEN_IDS),
      watchlist: orderByKnown([...watchlist], TARGET_TOKEN_IDS),
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
          Routes & targets
        </span>
        <span className="shrink-0 text-[10px] font-mono uppercase tracking-wider text-text-mut">
          Agent scope
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-4">
        <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_20rem]">
          <div className="space-y-3">
            <p className="max-w-3xl text-xs leading-relaxed text-text-lo">
              Choose what the agent is allowed to use. Green items can be used
              in real review plans now. Cyan items stay visible as tracked
              targets, but cannot execute until their route is connected.
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
              Current selection
            </div>
            <dl className="mt-3 space-y-2 text-xs">
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Wallet addresses
                </dt>
                <dd className="text-text-hi">
                  {selectedNetworkLabels || "No live network selected"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Can rebalance now
                </dt>
                <dd className="text-text-hi">
                  {executionNetworkLabels || "No execution rail ready"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Active targets
                </dt>
                <dd className="text-text-hi">
                  {selectedTokenLabels || "No target token selected"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Tracking only
                </dt>
                <dd className="text-text-lo">
                  {watchedTokenLabels || "No extra token watchlist"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="font-mono uppercase tracking-wider text-text-mut">
                  Next wallet sync
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
                              ? executionReady(network.blockchain)
                                ? "Selected for balance tracking and rebalances"
                                : "Selected for balance tracking only"
                              : executionReady(network.blockchain)
                                ? "Ready but not selected"
                                : "Wallet address ready; tracking only"
                            : tracked
                              ? "Requested for the next wallet sync"
                              : "No wallet address yet"}
                        </p>
                      </div>
                      <StatusPill
                        tone={
                          live && selected
                            ? "live"
                            : live
                              ? "muted"
                              : tracked
                                ? "agent"
                                : "muted"
                        }
                      >
                        {live
                          ? selected
                            ? executionReady(network.blockchain)
                              ? "Ready"
                              : "Track only"
                            : "Available"
                          : tracked
                            ? "Requested"
                            : "Not ready"}
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
                  className={`grid gap-3 rounded-sharp border p-3 sm:grid-cols-[minmax(0,1fr)_auto] ${
                    preferences.tokens.includes(token.id)
                      ? token.executable
                        ? "border-accent-pnl/45 bg-accent-pnl/5"
                        : "border-warn/45 bg-warn/5"
                      : preferences.watchlist.includes(token.id)
                        ? "border-accent-agent/40 bg-accent-agent/5"
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
                  <div className="flex flex-wrap items-center gap-2 sm:flex-col sm:items-end">
                    <StatusPill tone={tokenTone(token, preferences)}>
                      {tokenStatus(token, preferences)}
                    </StatusPill>
                    <button
                      type="button"
                      onClick={() => toggleToken(token.id)}
                      aria-pressed={preferences.tokens.includes(token.id)}
                      className="rounded-sharp border border-border-default bg-bg px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-text-lo transition-colors hover:border-accent-pnl hover:text-accent-pnl focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-border-hi"
                    >
                      {preferences.tokens.includes(token.id)
                        ? token.executable
                          ? "Remove"
                          : "Track only"
                        : token.executable
                          ? "Use target"
                          : "Track target"}
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
            Selection is an instruction, not a shortcut around execution safety.
            Today Aegis can prepare real reviews on Arc testnet and Base
            Sepolia, with USDC reserve as the active execution target. USYC
            yield, market, and FX tokens are tracked for planning, but reviews
            that need a disabled or unconnected route cannot be approved yet.{" "}
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
    tokens: liveNetworkIds.length > 0 ? EXECUTABLE_TOKEN_IDS : [],
    watchlist: TRACK_ONLY_TOKEN_IDS,
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
  const requestedTokens = normalizeTokenIds(preferences.tokens);
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
      normalizeTokenIds(preferences.watchlist).filter(
        (id) => knownTokens.has(id) && !tokens.includes(id),
      ),
      TARGET_TOKEN_IDS,
    ),
    updatedAt: preferences.updatedAt,
  };
}

function normalizeTokenIds(tokens: string[] | undefined): string[] {
  const selected = new Set(tokens ?? []);
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

function executionReady(blockchain: string): boolean {
  return EXECUTION_NETWORK_IDS.includes(blockchain);
}

function tokenExecutable(tokenId: string): boolean {
  return TOKENS.some((token) => token.id === tokenId && token.executable);
}

function tokenStatus(
  token: (typeof TOKENS)[number],
  preferences: RoutePreferences,
): string {
  if (preferences.tokens.includes(token.id)) {
    return token.executable ? "Ready" : "Track only";
  }
  if (preferences.watchlist.includes(token.id)) {
    return "Track only";
  }
  return token.state;
}

function tokenTone(
  token: (typeof TOKENS)[number],
  preferences: RoutePreferences,
): "live" | "warn" | "muted" | "agent" {
  if (preferences.tokens.includes(token.id)) {
    return token.executable ? "live" : "warn";
  }
  if (preferences.watchlist.includes(token.id)) {
    return "agent";
  }
  return "muted";
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
