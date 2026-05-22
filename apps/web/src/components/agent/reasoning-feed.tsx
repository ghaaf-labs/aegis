"use client";

import { useMemo, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Brain,
  RefreshCw,
  Zap,
  ShieldAlert,
  Cpu,
  Wifi,
  WifiOff,
  Wrench,
  HandIcon,
  AlertTriangle,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { usePortfolioStore, useActivePortfolio } from "@/stores/portfolio";
import { agentApi } from "@/lib/api";
import { formatCurrency, timeAgo } from "@/lib/utils";
import type {
  AgentAbstained,
  AgentDecision,
  AgentToolInvoked,
  AgentTrigger,
  CriticVerdict,
  MarketRegime,
} from "@/types";

const TRIGGER_LABELS: Record<AgentTrigger, string> = {
  drift_threshold: "Drift Alert",
  market_movement: "Market Signal",
  scheduled: "Scheduled",
  risk_breach: "Risk Breach",
  user_request: "Manual",
  regime_flip: "Regime Flip",
  abstain: "Abstain",
  peg_alert: "Peg Defense",
};

const TRIGGER_VARIANTS: Record<
  AgentTrigger,
  "warning" | "default" | "secondary" | "danger"
> = {
  drift_threshold: "warning",
  market_movement: "default",
  scheduled: "secondary",
  risk_breach: "danger",
  user_request: "secondary",
  regime_flip: "warning",
  abstain: "secondary",
  peg_alert: "danger",
};

const REGIME_LABEL: Record<MarketRegime, string> = {
  risk_on: "RISK-ON",
  neutral: "NEUTRAL",
  risk_off: "RISK-OFF",
};

const REGIME_CLASS: Record<MarketRegime, string> = {
  risk_on: "bg-cyan-500/15 text-accent-agent border border-cyan-500/30",
  neutral: "bg-white/8 text-text-hi border border-white/15",
  risk_off: "bg-rose-500/15 text-risk border border-rose-500/30",
};

export function AgentReasoningFeed() {
  const decisions = usePortfolioStore((s) => s.decisions);
  const setDecisions = usePortfolioStore((s) => s.setDecisions);
  const sseConnected = usePortfolioStore((s) => s.sseConnected);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const toolInvocations = usePortfolioStore((s) => s.toolInvocations);
  const abstains = usePortfolioStore((s) => s.abstains);
  const portfolio = useActivePortfolio();
  const [refreshing, setRefreshing] = useState(false);

  const handleRefresh = async () => {
    if (!portfolio || refreshing) return;
    setRefreshing(true);
    try {
      const fresh = await agentApi.decisions(portfolio.id);
      setDecisions(fresh);
    } catch {
      // best-effort
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <Card className="flex flex-col">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <CardTitle className="flex items-center gap-2">
          <Brain className="w-3.5 h-3.5 text-accent-agent" />
          Decision Log
        </CardTitle>
        <div className="flex items-center gap-2">
          <span
            className="flex items-center gap-1 text-[10px] text-text-mut"
            title={
              sseConnected
                ? "Realtime event stream connected"
                : "Realtime event stream reconnecting"
            }
          >
            {sseConnected ? (
              <Wifi className="w-3 h-3 text-accent-agent/80" />
            ) : (
              <WifiOff className="w-3 h-3 text-text-mut" />
            )}
            <span className="font-mono">
              {sseConnected ? "STREAM" : "OFFLINE"}
            </span>
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="text-text-mut hover:text-text-default h-7 px-2"
            onClick={() => void handleRefresh()}
            disabled={refreshing || !portfolio}
            title="Refresh decisions"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${refreshing ? "animate-spin" : ""}`}
            />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex-1 p-0 overflow-hidden">
        {(toolInvocations.length > 0 || abstains.length > 0) && (
          <LiveActivityStrip
            toolInvocations={toolInvocations.slice(0, 4)}
            abstains={abstains.slice(0, 2)}
          />
        )}
        <DecisionList
          decisions={decisions}
          currentState={{
            idleUsdc: unifiedUsdc,
            investedUsd: portfolio?.totalValueUsd ?? 0,
          }}
        />
      </CardContent>
    </Card>
  );
}

interface CurrentDecisionState {
  idleUsdc: number;
  investedUsd: number;
}

type DecisionView = "current" | "audit";

/**
 * Collapses runs of identical decisions ("Hold — portfolio is empty" repeated
 * 6×) into the most-recent one + a quiet "N similar prior decisions" footer.
 * Before this the feed would scroll forever with the same recommendation,
 * making the agent look stuck in a loop.
 */
function DecisionList({
  decisions,
  currentState,
}: {
  decisions: AgentDecision[];
  currentState: CurrentDecisionState;
}) {
  const [view, setView] = useState<DecisionView>("current");
  const currentDecisions = useMemo(
    () => decisions.filter((d) => !isAuditDecision(d, currentState)),
    [decisions, currentState],
  );
  const visibleDecisions = view === "current" ? currentDecisions : decisions;
  const groups = useMemo(
    () => groupRepeatedDecisions(visibleDecisions),
    [visibleDecisions],
  );
  const blockedCount = decisions.filter(isCriticBlocked).length;
  const legacyCount = decisions.filter(isLegacyLocalDecision).length;
  const staleCount = decisions.filter((d) =>
    isOutdatedForCurrentState(d, currentState),
  ).length;
  const auditCount = decisions.length - currentDecisions.length;

  if (decisions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center px-6">
        <Brain className="w-6 h-6 text-accent-agent/30 mb-3" />
        <p className="text-xs font-mono text-text-mut">
          No decisions yet — the agent will reason here when triggered by drift,
          a regime flip, or your manual request.
        </p>
      </div>
    );
  }

  return (
    <div className="overflow-y-auto max-h-[480px] scrollbar-thin">
      <div className="border-b border-white/4 px-5 py-3">
        <div
          className="grid grid-cols-2 border border-border-default bg-bg p-0.5 text-[10px] font-mono"
          role="tablist"
          aria-label="Decision log view"
        >
          <DecisionViewButton
            active={view === "current"}
            onClick={() => setView("current")}
          >
            Current · {currentDecisions.length}
          </DecisionViewButton>
          <DecisionViewButton
            active={view === "audit"}
            onClick={() => setView("audit")}
          >
            Full audit · {decisions.length}
          </DecisionViewButton>
        </div>
        {view === "current" && auditCount > 0 && (
          <p className="mt-2 text-[10px] font-mono leading-relaxed text-text-mut">
            {auditCount} historical, rejected, or cash-mismatched{" "}
            {auditCount === 1 ? "row is" : "rows are"} hidden from current
            guidance.
          </p>
        )}
      </div>

      {view === "current" && currentDecisions.length === 0 && (
        <div className="px-5 py-8 text-center">
          <p className="text-xs font-mono text-text-lo">
            No current executable guidance. Historical and rejected proposals
            are available in Full audit.
          </p>
        </div>
      )}

      {view === "audit" &&
        (blockedCount > 0 || legacyCount > 0 || staleCount > 0) && (
          <div className="border-b border-white/4 bg-rose-500/5 px-5 py-3 text-[10px] font-mono leading-relaxed text-text-lo">
            {staleCount > 0 && (
              <span className="text-warn">
                {staleCount} cash-mismatch{" "}
                {staleCount === 1 ? "proposal needs" : "proposals need"} a fresh
                plan
              </span>
            )}
            {staleCount > 0 && (blockedCount > 0 || legacyCount > 0) && (
              <span className="text-text-mut"> · </span>
            )}
            {blockedCount > 0 && (
              <span className="text-risk">
                {blockedCount} critic-blocked{" "}
                {blockedCount === 1 ? "proposal" : "proposals"}
              </span>
            )}
            {blockedCount > 0 && legacyCount > 0 && (
              <span className="text-text-mut"> · </span>
            )}
            {legacyCount > 0 && (
              <span>
                {legacyCount} old local{" "}
                {legacyCount === 1 ? "decision" : "decisions"} kept for audit
              </span>
            )}
          </div>
        )}

      <AnimatePresence initial={false}>
        {groups.map((g, i) => (
          <div key={g.head.id}>
            <DecisionRow
              decision={g.head}
              index={i}
              currentState={currentState}
            />
            {g.repeats > 0 && (
              <div className="px-5 py-2 border-b border-white/4 text-[10px] font-mono text-text-mut italic">
                + {g.repeats} earlier{" "}
                {g.repeats === 1 ? "decision" : "decisions"} with the same
                recommendation
              </div>
            )}
          </div>
        ))}
      </AnimatePresence>
    </div>
  );
}

function DecisionViewButton({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={
        "min-h-7 px-2 text-center transition-colors " +
        (active
          ? "bg-accent-agent text-black"
          : "text-text-lo hover:bg-raised hover:text-text-hi")
      }
    >
      {children}
    </button>
  );
}

/**
 * Buckets consecutive decisions sharing the same recommendation "shape".
 * Two decisions are considered the same if they're both no-trade holds (empty
 * trades array) — that's the loop the agent gets stuck in pre-deploy. Head =
 * most recent in the bucket; `repeats` = older copies collapsed.
 */
function groupRepeatedDecisions(decisions: AgentDecision[]) {
  const out: Array<{ head: AgentDecision; repeats: number }> = [];
  const noTradeShape = (d: AgentDecision) =>
    (d.recommendation?.trades?.length ?? 0) === 0;
  for (const d of decisions) {
    const shape = noTradeShape(d) ? "no-trade-hold" : `trades:${d.id}`;
    const last = out[out.length - 1];
    const lastShape = last
      ? noTradeShape(last.head)
        ? "no-trade-hold"
        : `trades:${last.head.id}`
      : null;
    if (last && lastShape === shape && shape === "no-trade-hold") {
      last.repeats += 1;
      continue;
    }
    out.push({ head: d, repeats: 0 });
  }
  return out;
}

/**
 * Above-the-fold realtime strip — every `agent.tool.invoked` / `agent.abstained`
 * SSE event lands here within the same animation frame the backend emits it.
 * Capped so it can't push the decisions list off-screen.
 */
function LiveActivityStrip({
  toolInvocations,
  abstains,
}: {
  toolInvocations: AgentToolInvoked[];
  abstains: AgentAbstained[];
}) {
  return (
    <div className="px-5 py-3 border-b border-white/4 bg-white/2">
      <p className="text-[10px] font-mono uppercase tracking-wider text-accent-agent/70 mb-2">
        Live agent activity
      </p>
      <div className="space-y-1.5">
        <AnimatePresence initial={false}>
          {abstains.map((a) => (
            <motion.div
              key={a.decidedAt}
              initial={{ opacity: 0, x: -8 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-2 text-[11px] font-mono text-warn/90"
            >
              <HandIcon className="w-3 h-3 shrink-0" />
              <span className="opacity-75">Abstained</span>
              <span className="opacity-50">·</span>
              <span className="truncate">{a.reason}</span>
              <span className="ml-auto text-[10px] opacity-50 shrink-0">
                {Math.round(a.confidence * 100)}%
              </span>
            </motion.div>
          ))}
          {toolInvocations.map((t) => (
            <motion.div
              key={`${t.invokedAt}-${t.toolName}`}
              initial={{ opacity: 0, x: -8 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-2 text-[11px] font-mono text-accent-agent/80"
            >
              <Wrench className="w-3 h-3 shrink-0" />
              <span className="opacity-75">{t.toolName}</span>
              <span className="opacity-50">·</span>
              <span className="truncate opacity-60">{t.resultPreview}</span>
              <span className="ml-auto text-[10px] opacity-50 shrink-0">
                {t.latencyMs}ms
              </span>
            </motion.div>
          ))}
        </AnimatePresence>
      </div>
    </div>
  );
}

function DecisionRow({
  decision,
  index,
  currentState,
}: {
  decision: AgentDecision;
  index: number;
  currentState: CurrentDecisionState;
}) {
  const trigger = knownTrigger(decision.triggeredBy);
  const triggerVariant = TRIGGER_VARIANTS[trigger] ?? "secondary";
  const triggerLabel = TRIGGER_LABELS[trigger] ?? trigger;
  const regime = knownRegime(decision.regime);
  const verdict = decision.criticVerdict;
  const blocked = isCriticBlocked(decision);
  const legacyLocal = isLegacyLocalDecision(decision);
  const outdated = isOutdatedForCurrentState(decision, currentState);
  const trades = decision.recommendation?.trades ?? [];
  const expectedCashNeeded = decisionExpectedCashNeeded(decision);
  const snapshotMismatch = decisionSnapshotMismatch(decision, currentState);

  return (
    <motion.div
      initial={{ opacity: 0, x: 8 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: Math.min(index * 0.04, 0.5) }}
      className={`px-5 py-4 border-b border-white/4 last:border-0 hover:bg-white/2 transition-colors group ${
        blocked || legacyLocal || outdated ? "bg-white/[0.015]" : ""
      }`}
    >
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="flex items-center gap-2 flex-wrap">
          <Badge variant={triggerVariant} className="text-[10px] px-1.5 py-0">
            {triggerLabel}
          </Badge>
          {regime && (
            <span
              className={`px-1.5 py-0.5 rounded-sm text-[10px] font-mono font-semibold tracking-tight ${REGIME_CLASS[regime]}`}
              title={`Regime classifier: ${REGIME_LABEL[regime]}`}
            >
              {REGIME_LABEL[regime]}
            </span>
          )}
          {decision.modelSlug && (
            <span
              className="flex items-center gap-1 px-1.5 py-0.5 rounded-sm text-[10px] font-mono border border-cyan-500/30 text-accent-agent/90 bg-cyan-500/5"
              title="Model that produced this decision"
            >
              <Cpu className="w-2.5 h-2.5" />
              {decision.modelSlug}
            </span>
          )}
          {blocked && (
            <span className="px-1.5 py-0.5 rounded-sm text-[10px] font-mono border border-rose-500/30 text-risk bg-rose-500/5">
              Blocked by critic
            </span>
          )}
          {legacyLocal && (
            <span className="px-1.5 py-0.5 rounded-sm text-[10px] font-mono border border-white/15 text-text-mut bg-white/5">
              Legacy local
            </span>
          )}
          {outdated && (
            <span className="flex items-center gap-1 px-1.5 py-0.5 rounded-sm text-[10px] font-mono border border-amber-500/30 text-warn bg-amber-500/5">
              <AlertTriangle className="w-2.5 h-2.5" />
              Needs fresh plan
            </span>
          )}
          <span className="text-[10px] text-text-mut">
            {timeAgo(decision.createdAt)}
          </span>
        </div>
        <ConfidencePill confidence={decision.confidence} />
      </div>

      <p
        className={`text-xs font-semibold mb-1.5 ${
          blocked || legacyLocal || outdated ? "text-text-lo" : "text-text-hi"
        }`}
      >
        {decision.recommendation?.summary ?? "Decision needs review"}
      </p>
      {blocked && (
        <p className="mb-2 text-[10px] font-mono text-risk">
          Not executable. The critic rejected this proposal; build a fresh plan
          before approving any movement.
        </p>
      )}
      {legacyLocal && (
        <p className="mb-2 text-[10px] font-mono text-text-mut">
          Historical test row only. It does not describe the current
          real-execution path.
        </p>
      )}
      {outdated && (
        <p className="mb-2 text-[10px] font-mono text-warn">
          {snapshotMismatch
            ? `Historical proposal. It was built from ${formatCurrency(snapshotMismatch.portfolioValueUsd)} positions and ${formatCurrency(snapshotMismatch.idleUsdc)} idle USDC; current state is ${formatCurrency(currentState.investedUsd)} positions and ${formatCurrency(currentState.idleUsdc)} idle USDC.`
            : `Historical proposal. It expects roughly ${formatCurrency(expectedCashNeeded)} deployable USDC, but current idle USDC is ${formatCurrency(currentState.idleUsdc)}.`}{" "}
          Build a fresh review plan before acting.
        </p>
      )}

      <p className="text-[11px] text-text-mut leading-relaxed line-clamp-3">
        {decision.reasoning || "No reasoning was returned with this decision."}
      </p>

      {trades.length > 0 && (
        <div className="mt-3 space-y-1.5">
          {trades.map((trade, ti) => {
            const action = normalizedTradeAction(trade);
            const valueUsd = tradeValueUsd(trade);
            return (
              <div
                // Real agent output doesn't always carry assetId/action —
                // fall back to symbol+index for a stable, unique key and keep
                // malformed historical rows from crashing the dashboard.
                key={`${decision.id}-${tradeSymbol(trade)}-${ti}`}
                className="flex items-center gap-2 text-[11px]"
              >
                <span
                  className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                    blocked || legacyLocal || action === "review"
                      ? "bg-white/5 text-text-mut"
                      : action === "buy"
                        ? "bg-cyan-500/15 text-accent-agent"
                        : "bg-red-500/15 text-risk"
                  }`}
                >
                  {action.toUpperCase()}
                </span>
                <span className="font-mono text-text-hi">
                  {tradeSymbol(trade)}
                </span>
                {valueUsd != null && (
                  <span className="font-mono text-text-lo tabular-nums">
                    {formatCurrency(valueUsd)}
                  </span>
                )}
                <span className="text-text-mut truncate">
                  {tradeReason(trade)}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {verdict && (verdict.demandsRevision || verdict.notes) && (
        <CriticLine verdict={verdict} />
      )}

      <TelemetryFooter decision={decision} />
    </motion.div>
  );
}

function isCriticBlocked(decision: AgentDecision) {
  return (
    decision.criticVerdict?.demandsRevision === true ||
    decision.criticVerdict?.verdict === "revised" ||
    decision.criticVerdict?.verdict === "veto"
  );
}

function isLegacyLocalDecision(decision: AgentDecision) {
  const haystack = `${decision.recommendation?.summary ?? ""} ${decision.reasoning ?? ""}`;
  return /mock decision|local\/demo|demo mock mode/i.test(haystack);
}

function isAuditDecision(
  decision: AgentDecision,
  currentState: CurrentDecisionState,
) {
  return (
    isCriticBlocked(decision) ||
    isLegacyLocalDecision(decision) ||
    isOutdatedForCurrentState(decision, currentState)
  );
}

function isOutdatedForCurrentState(
  decision: AgentDecision,
  currentState: CurrentDecisionState,
) {
  if (isLegacyLocalDecision(decision)) {
    return false;
  }

  const text = `${decision.recommendation?.summary ?? ""} ${decision.reasoning ?? ""}`;
  const mentionsCashDeployment = /\b(idle|deploy|cash|usdc|wallet)\b/i.test(
    text,
  );
  if (!mentionsCashDeployment) return false;

  const netExternalCashNeeded = decisionExpectedCashNeeded(decision);
  const hasMeaningfulTrade = netExternalCashNeeded > 1;
  const deterministicSnapshotMismatch = !!decisionSnapshotMismatch(
    decision,
    currentState,
  );

  return (
    deterministicSnapshotMismatch ||
    (hasMeaningfulTrade && netExternalCashNeeded > currentState.idleUsdc + 0.5)
  );
}

function decisionSnapshotMismatch(
  decision: AgentDecision,
  currentState: CurrentDecisionState,
) {
  const snapshot = decision.snapshot ?? {};
  if (snapshot.planner !== "deterministic") return null;
  const investedValueUsd = deterministicSnapshotInvestedUsd(snapshot);
  const idleUsdc = Number(snapshot.idleUsdc);
  const portfolioMismatch =
    Number.isFinite(investedValueUsd) &&
    Math.abs(investedValueUsd - currentState.investedUsd) > 0.5;
  const idleMismatch =
    Number.isFinite(idleUsdc) &&
    Math.abs(idleUsdc - currentState.idleUsdc) > 0.5;
  if (!portfolioMismatch && !idleMismatch) return null;
  return {
    portfolioValueUsd: Number.isFinite(investedValueUsd) ? investedValueUsd : 0,
    idleUsdc: Number.isFinite(idleUsdc) ? idleUsdc : 0,
  };
}

function deterministicSnapshotInvestedUsd(snapshot: Record<string, unknown>) {
  const explicit = Number(snapshot.investedValueUsd);
  if (Number.isFinite(explicit)) return explicit;
  const planValue = Number(snapshot.planValueUsd ?? snapshot.portfolioValueUsd);
  const idleUsdc = Number(snapshot.idleUsdc);
  if (Number.isFinite(planValue) && Number.isFinite(idleUsdc)) {
    return Math.max(0, planValue - idleUsdc);
  }
  return Number(snapshot.portfolioValueUsd);
}

function decisionTradeTotals(decision: AgentDecision) {
  let buyUsd = 0;
  let sellUsd = 0;

  for (const trade of decision.recommendation?.trades ?? []) {
    const valueUsd = tradeValueUsd(trade);
    if (valueUsd == null || valueUsd <= 0) continue;

    const action = normalizedTradeAction(trade);
    if (action === "buy") buyUsd += valueUsd;
    if (action === "sell") sellUsd += valueUsd;
  }

  return { buyUsd, sellUsd };
}

function decisionExpectedCashNeeded(decision: AgentDecision) {
  const { buyUsd, sellUsd } = decisionTradeTotals(decision);
  const structuredNeed = Math.max(0, buyUsd - sellUsd);
  if (structuredNeed > 0) return structuredNeed;

  const text = `${decision.recommendation?.summary ?? ""} ${decision.reasoning ?? ""}`;
  const targeted =
    text.match(
      /\b(?:deploy|invest|park|purchase|buy)\b[^.]{0,80}\$([0-9][0-9,]*(?:\.[0-9]+)?)/i,
    ) ?? text.match(/\$([0-9][0-9,]*(?:\.[0-9]+)?)\s+(?:idle\s+)?USDC/i);
  const targetedAmount = targeted?.[1];
  if (targetedAmount) {
    const amount = Number(targetedAmount.replace(/,/g, ""));
    if (Number.isFinite(amount) && amount > 0) return amount;
  }

  return 0;
}

type TradeLike = {
  action?: unknown;
  symbol?: unknown;
  reason?: unknown;
  valueUsd?: unknown;
  usdValue?: unknown;
};

function normalizedTradeAction(trade: TradeLike) {
  const action = trade.action;
  return action === "buy" || action === "sell" ? action : "review";
}

function tradeSymbol(trade: TradeLike) {
  return typeof trade.symbol === "string" && trade.symbol.trim()
    ? trade.symbol.trim().toUpperCase()
    : "UNKNOWN";
}

function tradeReason(trade: TradeLike) {
  return typeof trade.reason === "string" && trade.reason.trim()
    ? trade.reason
    : "Malformed historical trade row";
}

function tradeValueUsd(trade: TradeLike) {
  const raw = trade.valueUsd ?? trade.usdValue;
  const value = Number(raw);
  return Number.isFinite(value) && value > 0 ? value : null;
}

function ConfidencePill({ confidence }: { confidence: number }) {
  const pct = Math.round(safeConfidence(confidence) * 100);
  const tone =
    pct >= 75 ? "text-accent-agent" : pct >= 50 ? "text-warn" : "text-text-lo";
  return (
    <div
      className="flex items-center gap-1 shrink-0"
      title="Strategist confidence"
    >
      <Zap className={`w-3 h-3 ${tone}`} />
      <span className={`text-[10px] font-mono font-medium ${tone}`}>
        {pct}%
      </span>
    </div>
  );
}

function knownTrigger(value: unknown): AgentTrigger {
  return typeof value === "string" && value in TRIGGER_LABELS
    ? (value as AgentTrigger)
    : "user_request";
}

function knownRegime(value: unknown): MarketRegime | null {
  return typeof value === "string" && value in REGIME_LABEL
    ? (value as MarketRegime)
    : null;
}

function safeConfidence(value: unknown) {
  const n = Number(value);
  if (!Number.isFinite(n)) return 0;
  return Math.min(1, Math.max(0, n));
}

function CriticLine({ verdict }: { verdict: CriticVerdict }) {
  // Use cyan (agent accent) for approved, rose for revision requested.
  // This follows the strict design-system rule: green = money/PnL only,
  // cyan = agent surfaces.
  const tone = verdict.demandsRevision
    ? "text-risk border-rose-500/30 bg-rose-500/5"
    : "text-accent-agent border-cyan-500/30 bg-cyan-500/5";
  return (
    <div
      className={`mt-3 flex items-start gap-2 px-2 py-1.5 rounded border ${tone} text-[10px] leading-relaxed`}
    >
      <ShieldAlert className="w-3 h-3 mt-0.5 shrink-0" />
      <span className="font-mono">
        <span className="opacity-70">
          Critic ({Math.round((verdict.confidence ?? 0) * 100)}%):
        </span>{" "}
        {verdict.demandsRevision ? "Revision requested — " : "Approved — "}
        <span className="opacity-90">{verdict.notes || "(no notes)"}</span>
      </span>
    </div>
  );
}

function TelemetryFooter({ decision }: { decision: AgentDecision }) {
  const hasTelemetry =
    decision.promptTokens != null ||
    decision.completionTokens != null ||
    decision.latencyMs != null;
  if (!hasTelemetry) return null;
  return (
    <div className="mt-2 flex items-center gap-3 text-[10px] font-mono text-text-mut">
      {decision.promptTokens != null && (
        <span title="Prompt tokens">in {decision.promptTokens}</span>
      )}
      {decision.completionTokens != null && (
        <span title="Completion tokens">out {decision.completionTokens}</span>
      )}
      {decision.latencyMs != null && (
        <span title="End-to-end latency">{decision.latencyMs}ms</span>
      )}
    </div>
  );
}
