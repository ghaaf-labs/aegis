"use client";

import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { useEffect, useState } from "react";

import { pegApi } from "@/lib/api";
import { useApiQuery } from "@/lib/use-api-query";
import { usePortfolioStore } from "@/stores/portfolio";
import type { PegActionKind, PegAssetSymbol, PegRule } from "@/types";

const ASSETS: PegAssetSymbol[] = ["USDC", "EURC", "USYC"];
const ACTIONS: Array<{
  kind: PegActionKind;
  label: string;
  disabled?: boolean;
}> = [
  { kind: "alert", label: "Alert only" },
  { kind: "propose_rebalance", label: "Propose rebalance" },
  { kind: "auto_execute", label: "Auto-execute (locked)", disabled: true },
];

interface DraftRule {
  asset: PegAssetSymbol;
  thresholdPrice: number;
  windowSeconds: number;
  actionKind: PegActionKind;
  targetAsset: PegAssetSymbol | "";
}

const DEFAULT_DRAFT: DraftRule = {
  asset: "USDC",
  thresholdPrice: 0.995,
  windowSeconds: 300,
  actionKind: "alert",
  targetAsset: "",
};

/**
 * Peg-defense control surface. Lists current rules, creates new ones, and
 * surfaces an in-page alert log when `peg.alert` SSE events arrive. No toast
 * dependency — alerts pile into a top banner stack with a "dismiss all"
 * affordance for keyboard users.
 *
 * The strict colour rule still applies: agent / monitor controls use the cyan
 * agent variant; pause is the red `danger` variant.
 */
export function PegRuleEditor() {
  const rulesQuery = useApiQuery<PegRule[]>("peg.rules", () => pegApi.list());
  // Local overlay for optimistic mutations after onCreate / onPause /
  // onDelete. Falls back to the wrapper's cached data on initial load.
  const [localRules, setLocalRules] = useState<PegRule[] | null>(null);
  const rules = localRules ?? rulesQuery.data ?? [];
  const loading = rulesQuery.isLoading && !rulesQuery.data;
  const setRules = (next: PegRule[] | ((prev: PegRule[]) => PegRule[])) =>
    setLocalRules((prev) => {
      const base = prev ?? rulesQuery.data ?? [];
      return typeof next === "function"
        ? (next as (p: PegRule[]) => PegRule[])(base)
        : next;
    });
  const [error, setError] = useState<string | null>(
    rulesQuery.error?.message ?? null,
  );
  const [draft, setDraft] = useState<DraftRule>(DEFAULT_DRAFT);
  const [submitting, setSubmitting] = useState(false);
  // peg.alert SSE events are dispatched through the app-level SSE connection
  // in the portfolio store — no second connection needed (avoids JWT-in-URL).
  const storePegAlerts = usePortfolioStore((s) => s.pegAlerts);
  const [dismissed, setDismissed] = useState(false);
  const alerts = dismissed ? [] : storePegAlerts;
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const thresholdValid =
    Number.isFinite(draft.thresholdPrice) &&
    draft.thresholdPrice > 0 &&
    draft.thresholdPrice <= 1;
  const targetValid =
    draft.targetAsset === "" || draft.targetAsset !== draft.asset;
  const canCreate =
    !submitting &&
    thresholdValid &&
    targetValid &&
    draft.windowSeconds >= 0 &&
    draft.actionKind !== "auto_execute";

  // Mirror the wrapper's error into local state so the existing error
  // banner stays the only place the user sees failures.
  useEffect(() => {
    if (rulesQuery.error) setError(rulesQuery.error.message);
  }, [rulesQuery.error]);

  async function onCreate() {
    if (submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const created = await pegApi.create({
        asset: draft.asset,
        thresholdPrice: draft.thresholdPrice,
        windowSeconds: draft.windowSeconds,
        actionKind: draft.actionKind,
        targetAsset: draft.targetAsset === "" ? null : draft.targetAsset,
      });
      setRules((prev) => [created, ...prev]);
      setDraft(DEFAULT_DRAFT);
    } catch (e) {
      setError(e instanceof Error ? e.message : "failed to create rule");
    } finally {
      setSubmitting(false);
    }
  }

  async function onPauseToggle(rule: PegRule) {
    try {
      const updated = rule.pausedAt
        ? await pegApi.unpause(rule.id)
        : await pegApi.pause(rule.id);
      setRules((prev) => prev.map((r) => (r.id === rule.id ? updated : r)));
    } catch (e) {
      setError(e instanceof Error ? e.message : "failed to toggle pause");
    }
  }

  async function onDelete(ruleId: string) {
    try {
      await pegApi.remove(ruleId);
      setRules((prev) => prev.filter((r) => r.id !== ruleId));
      setConfirmDeleteId(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "failed to delete rule");
      setConfirmDeleteId(null);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {alerts.length > 0 ? (
        <BrutalCard>
          <BrutalCardHeader>
            <h3 className="text-sm font-semibold">Live peg alerts</h3>
            <BrutalButton
              variant="ghost"
              onClick={() => setDismissed(true)}
              aria-label="Dismiss all peg alerts"
            >
              Dismiss all
            </BrutalButton>
          </BrutalCardHeader>
          <BrutalCardBody>
            <ul className="flex flex-col gap-2">
              {alerts.map((a, idx) => (
                <li
                  key={`${a.ruleId}-${a.observedAt}-${idx}`}
                  className="flex items-center justify-between gap-2 text-xs"
                >
                  <span className="flex items-center gap-2">
                    <BrutalPill tone="warn">{a.asset}</BrutalPill>
                    <span>
                      ${a.observedPrice.toFixed(4)} &le; threshold $
                      {a.thresholdPrice.toFixed(4)}
                    </span>
                    <span className="text-text-mut">
                      &middot; {a.actionTaken.replace("_", " ")}
                    </span>
                  </span>
                  <time className="text-text-mut">
                    {new Date(a.observedAt).toLocaleTimeString()}
                  </time>
                </li>
              ))}
            </ul>
          </BrutalCardBody>
        </BrutalCard>
      ) : null}

      <BrutalCard>
        <BrutalCardHeader>
          <h3 className="text-sm font-semibold">New peg-defense rule</h3>
        </BrutalCardHeader>
        <BrutalCardBody>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold uppercase tracking-wider">
                Watch asset
              </span>
              <select
                className="bg-raised border-brutal border-border-default rounded-sharp px-2 py-1"
                value={draft.asset}
                onChange={(e) => {
                  const asset = e.target.value as PegAssetSymbol;
                  setDraft({
                    ...draft,
                    asset,
                    targetAsset:
                      draft.targetAsset === asset ? "" : draft.targetAsset,
                  });
                }}
              >
                {ASSETS.map((a) => (
                  <option key={a} value={a}>
                    {a}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold uppercase tracking-wider">
                Fire when price &le;
              </span>
              <input
                type="number"
                step="0.0001"
                min="0"
                max="1"
                className="bg-raised border-brutal border-border-default rounded-sharp px-2 py-1 font-mono"
                value={draft.thresholdPrice}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    thresholdPrice: Number(e.target.value) || 0,
                  })
                }
              />
              <span className="text-[11px] text-text-mut font-mono">
                Use a sub-$1 depeg trigger. Values above 1.0000 would alert
                while the asset is healthy, so they are blocked.
              </span>
              {!thresholdValid ? (
                <span className="text-[11px] text-risk font-mono">
                  Enter a value greater than 0 and at or below 1.0000.
                </span>
              ) : null}
            </label>

            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold uppercase tracking-wider">
                Window (seconds)
              </span>
              <input
                type="number"
                min="0"
                step="30"
                className="bg-raised border-brutal border-border-default rounded-sharp px-2 py-1 font-mono"
                value={draft.windowSeconds}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    windowSeconds: Number(e.target.value) || 0,
                  })
                }
              />
            </label>

            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold uppercase tracking-wider">
                Defensive target
              </span>
              <select
                className="bg-raised border-brutal border-border-default rounded-sharp px-2 py-1"
                value={draft.targetAsset}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    targetAsset: e.target.value as PegAssetSymbol | "",
                  })
                }
              >
                <option value="">— none —</option>
                {ASSETS.filter((a) => a !== draft.asset).map((a) => (
                  <option key={a} value={a}>
                    {a}
                  </option>
                ))}
              </select>
              {!targetValid ? (
                <span className="text-[11px] text-risk font-mono">
                  Pick a different destination asset.
                </span>
              ) : null}
            </label>

            <fieldset className="md:col-span-2 flex flex-col gap-2 text-xs">
              <legend className="font-semibold uppercase tracking-wider mb-1">
                Action when fired
              </legend>
              <div className="flex flex-wrap gap-3">
                {ACTIONS.map((a) => (
                  <label
                    key={a.kind}
                    className={`flex items-center gap-2 ${
                      a.disabled
                        ? "cursor-not-allowed text-text-mut"
                        : "cursor-pointer"
                    }`}
                  >
                    <input
                      type="radio"
                      name="actionKind"
                      value={a.kind}
                      checked={draft.actionKind === a.kind}
                      disabled={a.disabled}
                      onChange={() =>
                        setDraft({ ...draft, actionKind: a.kind })
                      }
                    />
                    <span>{a.label}</span>
                  </label>
                ))}
              </div>
              <p className="text-[11px] text-text-mut font-mono">
                Auto-execute is not enabled yet. Use Propose rebalance to draft
                the defensive plan for approval.
              </p>
            </fieldset>
          </div>

          <div className="mt-4 flex items-center justify-end gap-2">
            <BrutalButton
              variant="agent"
              onClick={onCreate}
              disabled={!canCreate}
              aria-label="Create peg-defense rule"
            >
              {submitting ? "Creating…" : "Create rule"}
            </BrutalButton>
          </div>
          {error ? (
            <p role="alert" className="mt-3 text-xs font-mono text-risk">
              {error}
            </p>
          ) : null}
        </BrutalCardBody>
      </BrutalCard>

      <BrutalCard>
        <BrutalCardHeader>
          <h3 className="text-sm font-semibold">Active rules</h3>
        </BrutalCardHeader>
        <BrutalCardBody>
          {loading ? (
            <p className="text-xs text-text-mut">Loading…</p>
          ) : rules.length === 0 ? (
            <p className="text-xs text-text-mut">
              No peg rules yet. Add one above.
            </p>
          ) : (
            <ul className="flex flex-col divide-y divide-border-default">
              {rules.map((rule) => (
                <li
                  key={rule.id}
                  className="flex items-center justify-between gap-3 py-2 text-xs"
                >
                  <div className="flex items-center gap-2">
                    <BrutalPill tone={rule.pausedAt ? "neutral" : "warn"}>
                      {rule.asset}
                    </BrutalPill>
                    <span className="font-mono">
                      &le; ${rule.thresholdPrice.toFixed(4)}
                    </span>
                    <span className="text-text-mut">
                      window {rule.windowSeconds}s &middot;{" "}
                      {rule.actionKind.replace("_", " ")}
                      {rule.targetAsset ? ` → ${rule.targetAsset}` : ""}
                    </span>
                    {rule.pausedAt ? (
                      <BrutalPill tone="neutral">PAUSED</BrutalPill>
                    ) : null}
                  </div>
                  <div className="flex items-center gap-2">
                    <BrutalButton
                      variant={rule.pausedAt ? "agent" : "danger"}
                      onClick={() => onPauseToggle(rule)}
                      aria-label={
                        rule.pausedAt ? "Resume peg rule" : "Pause peg rule"
                      }
                    >
                      {rule.pausedAt ? "Resume" : "Pause"}
                    </BrutalButton>
                    {confirmDeleteId === rule.id ? (
                      <span className="flex items-center gap-1">
                        <span className="text-[10px] font-mono text-text-lo">
                          Confirm?
                        </span>
                        <BrutalButton
                          variant="danger"
                          onClick={() => void onDelete(rule.id)}
                          aria-label="Confirm delete peg rule"
                        >
                          Yes
                        </BrutalButton>
                        <BrutalButton
                          variant="ghost"
                          onClick={() => setConfirmDeleteId(null)}
                          aria-label="Cancel delete"
                        >
                          No
                        </BrutalButton>
                      </span>
                    ) : (
                      <BrutalButton
                        variant="ghost"
                        onClick={() => setConfirmDeleteId(rule.id)}
                        aria-label="Delete peg rule"
                      >
                        Delete
                      </BrutalButton>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </BrutalCardBody>
      </BrutalCard>
    </div>
  );
}
