import { formatCurrency } from "@/lib/utils";
import type {
  AgentDecision,
  AgentTrigger,
  CriticVerdict,
  MarketRegime,
} from "@/types";

export type CurrentDecisionState = {
  idleUsdc: number;
  investedUsd: number;
};

export type DecisionView = "current" | "audit";

export type DecisionStatusTone = "agent" | "warn" | "risk" | "muted";

export type DecisionStatus = {
  body: string;
  label: string;
  tone: DecisionStatusTone;
};

export type TradeLike = {
  action?: unknown;
  symbol?: unknown;
  reason?: unknown;
  valueUsd?: unknown;
  usdValue?: unknown;
};

export const TRIGGER_LABELS: Record<AgentTrigger, string> = {
  drift_threshold: "Drift Alert",
  market_movement: "Market Signal",
  scheduled: "Scheduled",
  risk_breach: "Risk Breach",
  user_request: "User Review",
  regime_flip: "Regime Flip",
  abstain: "Abstain",
  peg_alert: "Peg Defense",
};

export const TRIGGER_VARIANTS: Record<
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

export const REGIME_LABEL: Record<MarketRegime, string> = {
  risk_on: "RISK-ON",
  neutral: "NEUTRAL",
  risk_off: "RISK-OFF",
};

export const REGIME_CLASS: Record<MarketRegime, string> = {
  risk_on: "border-border-default bg-bg text-accent-agent",
  neutral: "border-border-default bg-bg text-text-hi",
  risk_off: "border-border-default bg-bg text-risk",
};

export function groupRepeatedDecisions(decisions: AgentDecision[]) {
  const out: Array<{ head: AgentDecision; repeats: number }> = [];
  for (const decision of decisions) {
    const shape = decisionShape(decision);
    const last = out[out.length - 1];
    const lastShape = last ? decisionShape(last.head) : null;
    if (last && lastShape === shape) {
      last.repeats += 1;
      continue;
    }
    out.push({ head: decision, repeats: 0 });
  }
  return out;
}

function decisionShape(decision: AgentDecision) {
  const trades = decision.recommendation?.trades ?? [];
  if (trades.length === 0) return "no-trade-hold";
  return trades
    .map((trade) => {
      const action = normalizedTradeAction(trade);
      const value = tradeValueUsd(trade);
      return [
        action,
        tradeSymbol(trade),
        value == null ? "none" : value.toFixed(2),
        userFacingTradeReason(trade),
      ].join(":");
    })
    .join("|");
}

export function hasTradeLegs(decision: AgentDecision) {
  return (decision.recommendation?.trades ?? []).length > 0;
}

export function isCriticBlocked(decision: AgentDecision) {
  return (
    decision.criticVerdict != null && isRevisionVerdict(decision.criticVerdict)
  );
}

export function isRevisionVerdict(verdict: CriticVerdict) {
  return (
    verdict.demandsRevision === true ||
    verdict.verdict === "revised" ||
    verdict.verdict === "veto"
  );
}

export function isLegacyLocalDecision(decision: AgentDecision) {
  const haystack = `${decision.recommendation?.summary ?? ""} ${decision.reasoning ?? ""}`;
  return /mock decision|local\/demo|demo mock mode/i.test(haystack);
}

export function isAuditDecision(
  decision: AgentDecision,
  currentState: CurrentDecisionState,
) {
  return (
    isCriticBlocked(decision) ||
    isLegacyLocalDecision(decision) ||
    isOutdatedForCurrentState(decision, currentState)
  );
}

export function isOutdatedForCurrentState(
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

export function decisionSnapshotMismatch(
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

export function decisionTotalTradeUsd(decision: AgentDecision) {
  const { buyUsd, sellUsd } = decisionTradeTotals(decision);
  return buyUsd + sellUsd;
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

export function decisionExpectedCashNeeded(decision: AgentDecision) {
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

export function normalizedTradeAction(trade: TradeLike) {
  const action = trade.action;
  return action === "buy" || action === "sell" ? action : "review";
}

export function tradeSymbol(trade: TradeLike) {
  return typeof trade.symbol === "string" && trade.symbol.trim()
    ? trade.symbol.trim().toUpperCase()
    : "UNKNOWN";
}

export function decisionHeadline(decision: AgentDecision) {
  const trades = decision.recommendation?.trades ?? [];
  if (trades.length === 0) {
    return "No move needed";
  }

  const symbols = Array.from(new Set(trades.map(tradeSymbol))).filter(
    (symbol) => symbol !== "UNKNOWN",
  );
  const totalUsd = trades.reduce((sum, trade) => {
    const value = tradeValueUsd(trade);
    return sum + (value ?? 0);
  }, 0);

  if (totalUsd > 0 && symbols.length > 0) {
    return `Move ${formatCurrency(totalUsd)} to ${summarizeSymbols(symbols)}`;
  }

  return "Plan ready for review";
}

function summarizeSymbols(symbols: string[]) {
  const visible = symbols.slice(0, 5);
  const hiddenCount = symbols.length - visible.length;
  if (hiddenCount <= 0) return visible.join(" / ");
  return `${visible.join(" / ")} + ${hiddenCount} more`;
}

export function tradeActionLabel(
  action: ReturnType<typeof normalizedTradeAction>,
) {
  if (action === "buy") return "Move to";
  if (action === "sell") return "Move from";
  return "Review";
}

export function userFacingTradeReason(trade: TradeLike) {
  const raw =
    typeof trade.reason === "string" && trade.reason.trim()
      ? trade.reason.trim()
      : "";
  if (!raw) return "Needs review";

  const symbol = tradeSymbol(trade);
  const normalized = raw.toLowerCase();
  const usycLabel = symbol === "USYC" ? "USYC route" : "Yield route";
  const route = [
    {
      label: usycLabel,
      matches: normalized.includes("park_usyc") || normalized.includes("usyc"),
    },
    {
      label: "Wallet cash",
      matches:
        normalized.includes("gateway cash") || normalized.includes("wallet"),
    },
    { label: "Arc route", matches: normalized.includes("arc") },
    { label: "Base route", matches: normalized.includes("base") },
    {
      label: "Needs review",
      matches:
        normalized.includes("unsupported") || normalized.includes("malformed"),
    },
  ].find((candidate) => candidate.matches);

  return route?.label ?? raw;
}

export function tradeValueUsd(trade: TradeLike) {
  const raw = trade.valueUsd ?? trade.usdValue;
  const value = Number(raw);
  return Number.isFinite(value) && value > 0 ? value : null;
}

export function knownTrigger(value: unknown): AgentTrigger {
  return typeof value === "string" && value in TRIGGER_LABELS
    ? (value as AgentTrigger)
    : "user_request";
}

export function knownRegime(value: unknown): MarketRegime | null {
  return typeof value === "string" && value in REGIME_LABEL
    ? (value as MarketRegime)
    : null;
}

export function safeConfidence(value: unknown) {
  const n = Number(value);
  if (!Number.isFinite(n)) return 0;
  return Math.min(1, Math.max(0, n));
}

export function decisionStatusCopy({
  blocked,
  legacyLocal,
  outdated,
  showAuditDetails,
  tradesCount,
}: {
  blocked: boolean;
  legacyLocal: boolean;
  outdated: boolean;
  showAuditDetails: boolean;
  tradesCount: number;
}): DecisionStatus {
  if (blocked) {
    return {
      body: showAuditDetails
        ? "Rejected by the critic. Build a fresh plan before acting."
        : "This plan is blocked by the critic.",
      label: "Critic rejected",
      tone: "risk",
    };
  }
  if (outdated) {
    return {
      body: "Current wallet cash no longer matches this proposal.",
      label: "Needs fresh plan",
      tone: "warn",
    };
  }
  if (legacyLocal) {
    return {
      body: "Historical local row kept for audit only.",
      label: "Audit only",
      tone: "muted",
    };
  }
  if (tradesCount > 0) {
    return {
      body: "Approval remains the final execution gate.",
      label: "Executable after approval",
      tone: "agent",
    };
  }
  return {
    body: "No movement is proposed right now.",
    label: "No move needed",
    tone: "muted",
  };
}
