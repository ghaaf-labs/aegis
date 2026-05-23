/**
 * Per-decision proposal dismissal, persisted in localStorage.
 *
 * A Gate-1 allocation proposal that the user dismissed must stay dismissed —
 * an unapplied proposal previously re-opened on every store refetch / SSE
 * event / remount because dismissal lived in ephemeral component state. Keying
 * dismissal by `decision.id` makes "skip this proposal" durable, while a fresh
 * proposal (a new id, e.g. from Re-propose) still surfaces.
 */

const KEY = "aegis.dismissed_proposals";
/** Cap the stored set so it can't grow without bound. */
const MAX_REMEMBERED = 100;

function readIds(): string[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((v): v is string => typeof v === "string")
      : [];
  } catch {
    return [];
  }
}

export function isProposalDismissed(id: string | null | undefined): boolean {
  if (!id) return false;
  return readIds().includes(id);
}

export function dismissProposal(id: string | null | undefined): void {
  if (!id || typeof window === "undefined") return;
  const ids = readIds();
  if (ids.includes(id)) return;
  // Newest last; trim from the front so recent dismissals are retained.
  const next = [...ids, id].slice(-MAX_REMEMBERED);
  try {
    window.localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    /* storage full / disabled — dismissal is best-effort */
  }
}
