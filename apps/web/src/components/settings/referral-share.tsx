"use client";

import { useCallback, useEffect, useState } from "react";
import { Check, Copy, Gift, Users } from "lucide-react";
import { ProvenanceLine } from "@aegis/ui";
import { billingApi, walletApi, type ReferralsResponse } from "@/lib/api";
import { copyTextToClipboard } from "@/lib/clipboard";
import { handleForUserId } from "@/lib/md5";

/**
 * Referral-share card. The link is `<origin>/signup?ref=<handle>` where the
 * handle is `md5(user.id).slice(0, 8)` — the exact value the backend matches
 * when crediting a referral (see wallet auth `referrer_handle`) and the same
 * hash the leaderboard/diary expose. Earnings + payout status are real,
 * straight from `GET /billing/referrals` (settled in USDC via Nanopayments
 * under `BILLING_V2_ENABLED`).
 */
export function ReferralShare() {
  const [handle, setHandle] = useState<string | null>(null);
  const [referrals, setReferrals] = useState<ReferralsResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    walletApi
      .session()
      .then((session) => {
        if (!cancelled) setHandle(handleForUserId(session.user.id));
      })
      .catch(() => {
        if (!cancelled) setHandle(null);
      });
    billingApi
      .listReferrals()
      .then((r) => {
        if (!cancelled) setReferrals(r);
      })
      .catch(() => {
        // Referrals are best-effort here — an unauthenticated or
        // billing-v2-disabled response just leaves the totals hidden.
        if (!cancelled) setReferrals(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const origin =
    typeof window !== "undefined"
      ? window.location.origin
      : "https://aegis.app";
  const link = handle ? `${origin}/signup?ref=${handle}` : null;

  const copyLink = useCallback(async () => {
    if (!link) return;
    setCopyError(false);
    try {
      await copyTextToClipboard(link);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopyError(true);
    }
  }, [link]);

  const rows = referrals?.rows ?? [];

  return (
    <div className="rounded-sharp border-brutal border-border-default bg-bg p-4">
      <div className="flex items-start gap-3">
        <Gift className="mt-0.5 h-4 w-4 shrink-0 text-accent-pnl" />
        <div className="min-w-0 flex-1">
          <p className="font-mono text-sm font-semibold text-text-hi">
            Invite & earn
          </p>
          <p className="mt-1 font-mono text-[11px] leading-relaxed text-text-lo">
            Share your link. When a new account verifies, the reward settles to
            your Arc wallet in USDC.
          </p>

          {link ? (
            <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:items-center">
              <code
                className="min-w-0 flex-1 truncate rounded-sharp border-brutal border-border-default bg-surface px-3 py-2 font-mono text-[11px] text-text-hi"
                title={link}
              >
                {link}
              </code>
              <button
                type="button"
                onClick={() => void copyLink()}
                aria-label="Copy referral link"
                className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-pnl px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
              >
                {copied ? (
                  <>
                    <Check className="h-4 w-4" />
                    Copied
                  </>
                ) : (
                  <>
                    <Copy className="h-4 w-4" />
                    Copy link
                  </>
                )}
              </button>
            </div>
          ) : (
            <p className="mt-3 font-mono text-[11px] leading-relaxed text-text-mut">
              Sign in to load your referral link.
            </p>
          )}

          {copyError && (
            <p
              role="alert"
              className="mt-2 font-mono text-[11px] leading-relaxed text-risk"
            >
              Could not copy the link. Select it above and copy manually.
            </p>
          )}

          {referrals && (
            <>
              <dl className="mt-4 grid grid-cols-2 gap-4 font-mono text-[11px]">
                <div>
                  <dt className="text-text-lo">Paid out</dt>
                  <dd className="mt-0.5 text-accent-pnl">
                    ${referrals.totalPaidUsdc.toFixed(2)} USDC
                  </dd>
                </div>
                <div>
                  <dt className="text-text-lo">Pending</dt>
                  <dd className="mt-0.5 text-warn">
                    ${referrals.totalPendingUsdc.toFixed(2)} USDC
                  </dd>
                </div>
              </dl>

              {rows.length > 0 ? (
                <ul className="mt-3 max-h-40 space-y-1 overflow-y-auto">
                  {rows.slice(0, 10).map((r) => (
                    <li
                      key={r.id}
                      className="flex items-center justify-between border-b border-border-default pb-1 font-mono text-[11px] text-text-lo"
                    >
                      <span>{new Date(r.createdAt).toLocaleDateString()}</span>
                      <span
                        className={r.paidAt ? "text-accent-pnl" : "text-warn"}
                      >
                        {r.paidAt ? "paid" : "pending"} · $
                        {r.rewardUsdc.toFixed(2)}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="mt-3 flex items-center gap-2 font-mono text-[11px] leading-relaxed text-text-mut">
                  <Users className="h-3.5 w-3.5 shrink-0" />
                  No referrals yet. Share your link to start earning.
                </p>
              )}

              <div className="mt-3">
                <ProvenanceLine source="USDC referral rewards · Nanopayments" />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
