"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { taxApi, type TaxShareToken, type TaxSummary } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

const TTL_DEFAULT_DAYS = 30;

/**
 * Pro-tier tax export surface. The frontend never enforces the tier gate
 * — A3's middleware does — but the page degrades to "no tokens, no
 * portfolios" copy when the API returns empty / 404.
 */
export default function TaxSettingsPage() {
  const portfolios = usePortfolioStore((s) => s.portfolios);
  const activeId = usePortfolioStore((s) => s.activePortfolioId);

  const currentYear = new Date().getUTCFullYear();
  const years = useMemo(
    () => Array.from({ length: 6 }, (_, i) => currentYear - i),
    [currentYear],
  );

  const [portfolioId, setPortfolioId] = useState<string>("");
  const [year, setYear] = useState<number>(currentYear);
  const [mockExcluded, setMockExcluded] = useState<number | null>(null);
  const [shares, setShares] = useState<TaxShareToken[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [summary, setSummary] = useState<TaxSummary | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  useEffect(() => {
    if (!portfolioId && (activeId ?? portfolios[0]?.id)) {
      setPortfolioId(activeId ?? portfolios[0]!.id);
    }
  }, [activeId, portfolios, portfolioId]);

  const reloadShares = useCallback(async () => {
    try {
      const list = await taxApi.listShares();
      setShares(list.filter((t) => !t.revokedAt));
    } catch (e) {
      // 404 (flag off) or unauthed → empty state, not an error toast.
      setShares([]);
      if (e instanceof Error && !e.message.startsWith("404")) {
        setError(e.message);
      }
    }
  }, []);

  useEffect(() => {
    void reloadShares();
  }, [reloadShares]);

  // Refresh the wallet provenance whenever the selected portfolio/year flips.
  useEffect(() => {
    if (!portfolioId) {
      setSummary(null);
      return;
    }
    let cancelled = false;
    taxApi
      .summary(portfolioId, year)
      .then((s) => {
        if (!cancelled) setSummary(s);
      })
      .catch(() => {
        if (!cancelled) setSummary(null);
      });
    return () => {
      cancelled = true;
    };
  }, [portfolioId, year]);

  const performDownload = async () => {
    if (!portfolioId) return;
    setDownloading(true);
    setError(null);
    try {
      const { mockExcluded: m } = await taxApi.downloadCsv(portfolioId, year);
      setMockExcluded(m);
      setConfirmOpen(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "download failed");
    } finally {
      setDownloading(false);
    }
  };

  const onDownloadClick = () => {
    if (!portfolioId) return;
    setConfirmOpen(true);
  };

  const onCreateShare = async () => {
    if (!portfolioId) return;
    setCreating(true);
    setError(null);
    try {
      await taxApi.createShare(portfolioId, year, TTL_DEFAULT_DAYS);
      await reloadShares();
    } catch (e) {
      setError(e instanceof Error ? e.message : "share creation failed");
    } finally {
      setCreating(false);
    }
  };

  const onRevoke = async (tokenId: string) => {
    setError(null);
    try {
      await taxApi.revokeShare(tokenId);
      await reloadShares();
    } catch (e) {
      setError(e instanceof Error ? e.message : "revoke failed");
    }
  };

  return (
    <div className="max-w-[1100px] mx-auto space-y-6">
      <header className="space-y-1">
        <h1 className="text-2xl font-mono font-semibold text-text-hi">
          Tax export
        </h1>
        <p className="text-sm text-text-lo max-w-2xl">
          IRS 1099-DA-ready CSV per portfolio, including stablecoin↔stablecoin
          dispositions (USDC↔EURC FX gain/loss). Pro feature.
        </p>
      </header>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">Download CSV</span>
          <BrutalPill tone="pnl">1099-DA</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <label className="block text-xs font-mono uppercase text-text-lo">
              Portfolio
              <select
                value={portfolioId}
                onChange={(e) => setPortfolioId(e.target.value)}
                className="mt-1 block w-full bg-surface border-brutal border-border-default rounded-sharp px-2 py-1 text-sm text-text-default"
              >
                {portfolios.length === 0 ? (
                  <option value="">(no portfolios)</option>
                ) : (
                  portfolios.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))
                )}
              </select>
            </label>
            <label className="block text-xs font-mono uppercase text-text-lo">
              Year
              <select
                value={year}
                onChange={(e) => setYear(Number(e.target.value))}
                className="mt-1 block w-full bg-surface border-brutal border-border-default rounded-sharp px-2 py-1 text-sm text-text-default"
              >
                {years.map((y) => (
                  <option key={y} value={y}>
                    {y}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {summary && summary.wallets.length > 0 && (
            <div className="border-brutal border-border-default bg-raised p-3 space-y-2">
              <p className="text-xs font-mono uppercase tracking-widest text-text-lo">
                Coverage
              </p>
              <ul className="space-y-1">
                {summary.wallets.map((w) => (
                  <li
                    key={`${w.chain}-${w.address}`}
                    className="flex items-center justify-between gap-2 text-xs font-mono"
                  >
                    <span className="text-text-hi">
                      {w.chain.toUpperCase()} ·{" "}
                      <span className="text-text-lo">
                        {w.address.slice(0, 6)}…{w.address.slice(-4)}
                      </span>
                    </span>
                    <span className="text-text-lo">
                      {w.lotCount} lots ·{" "}
                      {w.lastSyncedAt
                        ? new Date(w.lastSyncedAt).toLocaleDateString()
                        : "no sync"}
                    </span>
                  </li>
                ))}
              </ul>
              <p className="text-[10px] text-text-mut leading-relaxed">
                {summary.caveat}
              </p>
            </div>
          )}

          <BrutalButton
            variant="pnl"
            onClick={onDownloadClick}
            disabled={!portfolioId || downloading}
          >
            {downloading ? "Preparing…" : "Download CSV"}
          </BrutalButton>

          {mockExcluded !== null && mockExcluded > 0 ? (
            <p className="text-xs font-mono text-text-lo">
              {mockExcluded} mock entries excluded · only real settled moves
              appear in the export.
            </p>
          ) : null}
        </BrutalCardBody>
      </BrutalCard>

      {confirmOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
          role="dialog"
          aria-modal="true"
          aria-labelledby="tax-confirm-title"
        >
          <BrutalCard className="max-w-md w-full">
            <BrutalCardHeader>
              <span
                id="tax-confirm-title"
                className="text-sm font-mono text-text-hi"
              >
                Confirm tax export
              </span>
            </BrutalCardHeader>
            <BrutalCardBody className="space-y-3 text-xs text-text-lo">
              <p>
                This CSV covers{" "}
                <span className="text-text-hi font-mono">
                  {summary?.wallets.length ?? 0} wallets
                </span>{" "}
                and{" "}
                <span className="text-text-hi font-mono">
                  {summary?.totalLotCount ?? 0} lots
                </span>{" "}
                across portfolio {portfolioId.slice(0, 8)}… for year {year}.
                Includes USDC↔EURC FX dispositions per the 2026 1099-DA final
                rules (wallet-by-wallet basis).
              </p>
              <div className="flex gap-2 justify-end pt-2">
                <BrutalButton
                  variant="ghost"
                  onClick={() => setConfirmOpen(false)}
                >
                  Cancel
                </BrutalButton>
                <BrutalButton
                  variant="pnl"
                  onClick={() => void performDownload()}
                  disabled={downloading}
                >
                  {downloading ? "Preparing…" : "Download"}
                </BrutalButton>
              </div>
            </BrutalCardBody>
          </BrutalCard>
        </div>
      )}

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">
            Share with accountant
          </span>
          <BrutalPill tone="agent">read-only · {TTL_DEFAULT_DAYS}d</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody className="space-y-4">
          <p className="text-xs text-text-lo">
            Create a signed URL your accountant can hit without logging in.
            Revoke any time.
          </p>
          <BrutalButton
            variant="agent"
            onClick={onCreateShare}
            disabled={!portfolioId || creating}
          >
            {creating ? "Creating…" : "Create share link"}
          </BrutalButton>

          {shares.length > 0 ? (
            <ul className="space-y-2">
              {shares.map((s) => (
                <li
                  key={s.id}
                  className="flex items-center justify-between gap-2 border-brutal border-border-default rounded-sharp px-3 py-2 bg-raised text-xs font-mono"
                >
                  <div className="flex flex-col min-w-0">
                    <span className="text-text-hi truncate">
                      Year {s.year} · expires{" "}
                      {new Date(s.expiresAt).toISOString().slice(0, 10)}
                    </span>
                    <button
                      type="button"
                      className="text-accent-agent hover:underline truncate text-left"
                      onClick={() => {
                        void navigator.clipboard.writeText(s.token);
                      }}
                      title="Copy token"
                    >
                      {s.token.slice(0, 10)}…
                    </button>
                  </div>
                  <BrutalButton
                    variant="danger"
                    onClick={() => onRevoke(s.id)}
                    className="text-xs"
                  >
                    Revoke
                  </BrutalButton>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-xs text-text-lo">No active share links.</p>
          )}
        </BrutalCardBody>
      </BrutalCard>

      {error ? (
        <p className="text-xs font-mono text-risk">Error: {error}</p>
      ) : null}
    </div>
  );
}
