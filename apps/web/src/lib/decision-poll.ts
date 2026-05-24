import { agentApi } from "@/lib/api";
import type { AgentDecision } from "@/types";

const POLL_INTERVAL_MS = 2000;
// Cap the wait so a wedged/slow job can't hang the UI. The backend's
// per-attempt budget × retries sits comfortably under this; on a timeout the
// caller shows a retry, and the boot reconciler fails any job orphaned by a
// restart.
const MAX_WAIT_MS = 4 * 60 * 1000;

/**
 * Poll an agent decision until its async inference job reaches a terminal
 * state. Resolves with the `ready` decision; throws on `failed`/timeout so the
 * caller can surface a retry. A decision with no `status` is a legacy or
 * synchronous row — already terminal.
 *
 * `isMounted` lets a component abort the loop on unmount (defaults to always).
 */
export async function pollDecisionReady(
  decisionId: string,
  isMounted: () => boolean = () => true,
  opts: { intervalMs?: number; maxWaitMs?: number } = {},
): Promise<AgentDecision> {
  const intervalMs = opts.intervalMs ?? POLL_INTERVAL_MS;
  const deadline = Date.now() + (opts.maxWaitMs ?? MAX_WAIT_MS);
  while (Date.now() < deadline && isMounted()) {
    const decision = await agentApi.decisionById(decisionId);
    if (decision.status === "ready" || decision.status === undefined) {
      return decision;
    }
    if (decision.status === "failed") {
      throw new Error(decision.error ?? "agent job failed");
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
  throw new Error("agent job timed out");
}
