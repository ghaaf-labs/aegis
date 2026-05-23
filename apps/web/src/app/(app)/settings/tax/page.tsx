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

  const [error, setError] = useState<string | null>(null);
  const copyShare = useCallback(async (id: string, shareUrl: string) => {
    try {
      await copyText(shareUrl);
      setError(null);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 1800);
    } catch {
      setError("Could not copy the link. Open the report and copy it there.");
    }
  }, []);
  const [downloading, setDownloading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [summary, setSummary] = useState<TaxSummary | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const selectedPortfolio = portfolios.find((p) => p.id === portfolioId);
  const selectedShare = shares.find(
    (s) => s.portfolioId === portfolioId && s.year === year,
  );
  const sharePortfolioName = useCallback(
    (portfolioId: string) =>
      portfolios.find((p) => p.id === portfolioId)?.name ?? "Portfolio",
    [portfolios],
  );

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
      setError(e instanceof Error ? e.message : "Could not download report.");
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
      setError(e instanceof Error ? e.message : "Could not create share link.");
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
      setError(e instanceof Error ? e.message : "Could not revoke share link.");
    }
  };

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
          Tax center
        </h1>
        <p className="text-sm text-text-lo mt-1">
          Download settled activity or share a read-only accountant link.
        </p>
      </div>

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">
            Download tax report
          </span>
          <BrutalPill tone="pnl">Settled moves only</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <label className="block text-xs font-mono uppercase text-text-lo">
              Portfolio
              <select
                value={portfolioId}
                onChange={(e) => setPortfolioId(e.target.value)}
                className="mt-1 block min-h-9 w-full rounded-sharp border-brutal border-border-default bg-surface px-2 py-1 text-sm text-text-default"
              >
                {!mounted ? (
                  <option value="">Loading…</option>
                ) : portfolios.length === 0 ? (
                  <option value="">No portfolios yet</option>
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
                className="mt-1 block min-h-9 w-full rounded-sharp border-brutal border-border-default bg-surface px-2 py-1 text-sm text-text-default"
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
                      {taxWalletStatus(w.lotCount, w.lastSyncedAt)}
                    </span>
                  </li>
                ))}
              </ul>
              <p className="text-[10px] text-text-mut leading-relaxed">
                Wallets are shown so your accountant can match the report to
                on-chain addresses.
              </p>
            </div>
          )}

          <div className="border border-border-default bg-surface p-3 text-xs text-text-lo leading-relaxed">
            <div className="flex items-start gap-2">
              <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-accent-agent" />
              <p>
                The report includes settled Aegis activity only. Use the wallet
                list above to reconcile anything you did outside Aegis.
              </p>
            </div>
          </div>

          <BrutalButton
            variant="pnl"
            onClick={onDownloadClick}
            disabled={!portfolioId || downloading}
          >
            {downloading ? "Preparing…" : "Review & download report"}
          </BrutalButton>

          {mockExcluded !== null && mockExcluded > 0 ? (
            <p className="text-xs font-mono text-text-lo">
              {mockExcluded} draft or test rows excluded. Only settled moves
              appear in the report.
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
                Review tax report
              </span>
            </BrutalCardHeader>
            <BrutalCardBody className="space-y-3 text-xs text-text-lo">
              <p>
                This report covers{" "}
                <span className="text-text-hi font-mono">
                  {summary?.wallets.length ?? 0} wallets
                </span>{" "}
                and{" "}
                <span className="text-text-hi font-mono">
                  {summary?.totalLotCount ?? 0} records
                </span>{" "}
                for {selectedPortfolio?.name ?? "this portfolio"} in {year}.
                Wallet addresses are included for reconciliation.
              </p>
              <p>
                Only settled moves with a transaction reference are included.
                Unfinished rows are left out and reported after download.
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
          <BrutalPill tone="agent">{TTL_DEFAULT_DAYS} day link</BrutalPill>
        </BrutalCardHeader>
        <BrutalCardBody className="space-y-4">
          <p className="text-xs text-text-lo">
            Create a revocable read-only link for this portfolio and year. It
            does not expose the rest of your Aegis account.
          </p>
          <BrutalButton
            variant="agent"
            onClick={onCreateShare}
            disabled={!portfolioId || creating || !!selectedShare}
          >
            {creating
              ? "Creating…"
              : selectedShare
                ? "Link active"
                : "Create accountant link"}
          </BrutalButton>
          {selectedShare ? (
            <p className="text-xs text-text-lo">
              A link already exists for this portfolio and year. Copy it below
              or revoke it before creating a new one.
            </p>
          ) : null}

          {shares.length > 0 ? (
            <ul className="space-y-2">
              {shares.map((s) => (
                <li
                  key={s.id}
                  className="grid gap-3 border-brutal border-border-default rounded-sharp bg-raised px-3 py-2 text-xs font-mono sm:grid-cols-[minmax(0,1fr)_auto]"
                >
                  <div className="flex flex-col min-w-0">
                    <span className="text-text-hi">
                      {sharePortfolioName(s.portfolioId)} · {s.year} · expires{" "}
                      {new Date(s.expiresAt).toISOString().slice(0, 10)}
                    </span>
                    <span className="text-text-lo">
                      Read-only tax report link
                    </span>
                  </div>
                  <div className="flex flex-wrap items-center gap-2 sm:justify-end">
                    <a
                      href={s.shareUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex min-h-[32px] items-center gap-1 rounded-sharp border border-accent-agent/40 bg-accent-agent/5 px-2 text-accent-agent hover:bg-accent-agent/10"
                    >
                      Open report
                      <ExternalLink className="w-3 h-3 shrink-0" />
                    </a>
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
                  </div>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-xs text-text-lo">No active share links.</p>
          )}
        </BrutalCardBody>
      </BrutalCard>

      {error ? (
        <p className="text-xs font-mono text-risk" aria-live="polite">
          {error}
        </p>
      ) : null}
    </div>
  );
}

function taxWalletStatus(lotCount: number, lastSyncedAt: string | null) {
  if (lotCount === 0) return "No settled rows yet";
  const syncDate = lastSyncedAt
    ? new Date(lastSyncedAt).toLocaleDateString()
    : "sync pending";
  return `${lotCount} records · ${syncDate}`;
}

async function copyText(value: string) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value);
      return;
    } catch {
      // Fallback below handles embedded browsers that block clipboard writes.
    }
  }

  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("copy failed");
}
