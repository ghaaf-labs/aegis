"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, Copy, ExternalLink, ShieldCheck } from "lucide-react";
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

  const [mounted, setMounted] = useState(false);
  const [portfolioId, setPortfolioId] = useState<string>("");
  const [year, setYear] = useState<number>(currentYear);
  const [mockExcluded, setMockExcluded] = useState<number | null>(null);
  const [shares, setShares] = useState<TaxShareToken[]>([]);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const copyShare = useCallback(async (id: string, shareUrl: string) => {
    try {
      await navigator.clipboard.writeText(shareUrl);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 1800);
    } catch {
      /* clipboard blocked */
    }
  }, []);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [summary, setSummary] = useState<TaxSummary | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

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
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
          Tax center
        </h1>
        <p className="text-sm text-text-lo mt-1">
          Accountant CSV aligned to 1099-DA fields, generated per portfolio from
          real settled Aegis moves. Stablecoin↔stablecoin swaps are included
          when the executor recorded a real transaction reference.
        </p>
      </div>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">Download CSV</span>
          <BrutalPill tone="pnl">1099-DA CSV</BrutalPill>
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
                {!mounted ? (
                  <option value="">Loading…</option>
                ) : portfolios.length === 0 ? (
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
                Wallets are listed for accountant reconciliation and coverage
                checks.
              </p>
            </div>
          )}

          <div className="border border-border-default bg-surface p-3 text-xs text-text-lo leading-relaxed">
            <div className="flex items-start gap-2">
              <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-accent-agent" />
              <p>
                Basis is calculated at portfolio level with FIFO where Aegis has
                cost-basis lots. Wallet addresses are shown for reconciliation;
                this version does not split basis by wallet or chain.
              </p>
            </div>
          </div>

          <BrutalButton
            variant="pnl"
            onClick={onDownloadClick}
            disabled={!portfolioId || downloading}
          >
            {downloading ? "Preparing…" : "Review & download CSV"}
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
                across portfolio {portfolioId.slice(0, 8)}… for year {year}. It
                uses portfolio-level FIFO basis and includes wallet addresses
                for accountant reconciliation. It does not split basis by wallet
                or chain.
              </p>
              <p>
                The export only includes settled rows with transaction
                references; mock or unfinished execution rows are excluded and
                reported after download.
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
          <BrutalPill tone="agent">signed CSV · {TTL_DEFAULT_DAYS}d</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody className="space-y-4">
          <p className="text-xs text-text-lo">
            Create a revocable read-only URL that downloads this portfolio and
            year as a CSV without exposing the rest of your Aegis session.
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
                    <a
                      href={s.shareUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="text-accent-agent hover:underline truncate text-left flex items-center gap-1"
                    >
                      {s.shareUrl}
                      <ExternalLink className="w-3 h-3 shrink-0" />
                    </a>
                  </div>
                  <button
                    type="button"
                    className="inline-flex min-h-[32px] shrink-0 items-center gap-1 rounded-sharp border border-border-default bg-bg px-2 text-text-lo hover:border-border-hi hover:text-text-hi"
                    onClick={() => void copyShare(s.id, s.shareUrl)}
                    title="Copy share URL"
                    aria-label="Copy share URL"
                  >
                    {copiedId === s.id ? (
                      <>
                        <Check className="w-3 h-3 text-accent-pnl" />
                        Copied
                      </>
                    ) : (
                      <>
                        <Copy className="w-3 h-3" />
                        Copy link
                      </>
                    )}
                  </button>
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
