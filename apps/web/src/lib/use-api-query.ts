"use client";

import { useCallback, useEffect, useRef, useState } from "react";

interface CacheEntry<T> {
  data: T;
  storedAt: number;
}

interface Options {
  /**
   * Re-fetch on next mount when the cached value is older than this many
   * milliseconds. Defaults to 30 000ms — long enough that paging back to
   * the dashboard re-uses the previous response, short enough that a
   * paused user still sees fresh prices when they return.
   */
  staleAfterMs?: number;
  /** Disable the query without unmounting the component. */
  enabled?: boolean;
}

interface Result<T> {
  data: T | undefined;
  error: Error | null;
  isLoading: boolean;
  refetch: () => void;
}

const cache = new Map<string, CacheEntry<unknown>>();

/**
 * Tiny read-only data-fetching wrapper. Backed by a process-global Map
 * cache so two components asking for the same key on the same page share
 * the response. Writes should call the apiClient methods directly; this
 * hook is for reads.
 *
 * The cache is intentionally not invalidated on a route change — if you
 * need fresh data on every mount, set `staleAfterMs: 0`. If you mutated
 * server state and need to refresh, call the returned `refetch()` after
 * the mutation resolves.
 */
export function useApiQuery<T>(
  key: string,
  fetcher: () => Promise<T>,
  options: Options = {},
): Result<T> {
  const { staleAfterMs = 30_000, enabled = true } = options;
  const [data, setData] = useState<T | undefined>(
    () => (cache.get(key)?.data as T | undefined) ?? undefined,
  );
  const [error, setError] = useState<Error | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  // Hold the latest fetcher in a ref so we don't re-fetch every render when
  // the caller passes an inline arrow.
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const run = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const next = await fetcherRef.current();
      cache.set(key, { data: next as unknown, storedAt: Date.now() });
      setData(next);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setIsLoading(false);
    }
  }, [key]);

  useEffect(() => {
    if (!enabled) return;
    const hit = cache.get(key);
    const fresh = hit && Date.now() - hit.storedAt < staleAfterMs;
    if (fresh) {
      setData(hit.data as T);
      return;
    }
    void run();
  }, [key, enabled, staleAfterMs, run]);

  return { data, error, isLoading, refetch: run };
}
