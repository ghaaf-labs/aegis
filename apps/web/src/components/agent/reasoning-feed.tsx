"use client";

import { motion, AnimatePresence } from "framer-motion";
import {
  Brain,
  RefreshCw,
  ChevronRight,
  Zap,
  ShieldAlert,
  Cpu,
  Wifi,
  WifiOff,
  Wrench,
  HandIcon,
} from "lucide-react";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { usePortfolioStore } from "@/stores/portfolio";
import { timeAgo } from "@/lib/utils";
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
};

const REGIME_LABEL: Record<MarketRegime, string> = {
  risk_on: "RISK-ON",
  neutral: "NEUTRAL",
  risk_off: "RISK-OFF",
};

const REGIME_CLASS: Record<MarketRegime, string> = {
  risk_on: "bg-emerald-500/15 text-emerald-300 border border-emerald-500/30",
  neutral: "bg-white/8 text-white border border-white/15",
  risk_off: "bg-rose-500/15 text-rose-300 border border-rose-500/30",
};

export function AgentReasoningFeed() {
  const decisions = usePortfolioStore((s) => s.decisions);
  const sseConnected = usePortfolioStore((s) => s.sseConnected);
  const toolInvocations = usePortfolioStore((s) => s.toolInvocations);
  const abstains = usePortfolioStore((s) => s.abstains);

  return (
    <Card className="flex flex-col">
      <CardHeader className="flex flex-row items-center justify-between pb-3">
        <CardTitle className="flex items-center gap-2">
          <Brain className="w-3.5 h-3.5 text-cyan-400" />
          AI Reasoning
        </CardTitle>
        <div className="flex items-center gap-2">
          <span
            className="flex items-center gap-1 text-[10px] text-gray-500"
            title={sseConnected ? "Live feed connected" : "Reconnecting…"}
          >
            {sseConnected ? (
              <Wifi className="w-3 h-3 text-emerald-400/80" />
            ) : (
              <WifiOff className="w-3 h-3 text-gray-500" />
            )}
            <span className="font-mono">
              {sseConnected ? "LIVE" : "OFFLINE"}
            </span>
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="text-gray-500 hover:text-gray-300 h-7 px-2"
          >
            <RefreshCw className="w-3.5 h-3.5" />
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
        <div className="overflow-y-auto max-h-[480px] scrollbar-thin">
          <AnimatePresence initial={false}>
            {decisions.map((decision, i) => (
              <DecisionRow key={decision.id} decision={decision} index={i} />
            ))}
          </AnimatePresence>
        </div>
      </CardContent>
    </Card>
  );
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
      <p className="text-[10px] font-mono uppercase tracking-wider text-cyan-300/70 mb-2">
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
              className="flex items-center gap-2 text-[11px] font-mono text-amber-300/90"
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
              className="flex items-center gap-2 text-[11px] font-mono text-cyan-200/90"
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
}: {
  decision: AgentDecision;
  index: number;
}) {
  const trigger: AgentTrigger = decision.triggeredBy;
  const triggerVariant = TRIGGER_VARIANTS[trigger] ?? "secondary";
  const triggerLabel = TRIGGER_LABELS[trigger] ?? trigger;
  const regime = decision.regime;
  const verdict = decision.criticVerdict;

  return (
    <motion.div
      initial={{ opacity: 0, x: 8 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: Math.min(index * 0.04, 0.5) }}
      className="px-5 py-4 border-b border-white/4 last:border-0 hover:bg-white/2 transition-colors cursor-pointer group"
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
              className="flex items-center gap-1 px-1.5 py-0.5 rounded-sm text-[10px] font-mono border border-cyan-500/30 text-cyan-300/90 bg-cyan-500/5"
              title="Model that produced this decision"
            >
              <Cpu className="w-2.5 h-2.5" />
              {decision.modelSlug}
            </span>
          )}
          <span className="text-[10px] text-gray-600">
            {timeAgo(decision.createdAt)}
          </span>
        </div>
        <ConfidencePill confidence={decision.confidence} />
      </div>

      <p className="text-xs font-semibold text-white mb-1.5">
        {decision.recommendation.summary}
      </p>

      <p className="text-[11px] text-gray-500 leading-relaxed line-clamp-3">
        {decision.reasoning}
      </p>

      {decision.recommendation.trades.length > 0 && (
        <div className="mt-3 space-y-1.5">
          {decision.recommendation.trades.map((trade, ti) => (
            <div
              // Real agent output doesn't carry assetId — fall back to
              // symbol+index for a stable, unique key.
              key={`${decision.id}-${trade.symbol ?? "x"}-${ti}`}
              className="flex items-center gap-2 text-[11px]"
            >
              <span
                className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                  trade.action === "buy"
                    ? "bg-emerald-500/15 text-emerald-400"
                    : "bg-red-500/15 text-red-400"
                }`}
              >
                {trade.action.toUpperCase()}
              </span>
              <span className="font-mono text-white">{trade.symbol}</span>
              <span className="text-gray-500 truncate">{trade.reason}</span>
            </div>
          ))}
        </div>
      )}

      {verdict && (verdict.demandsRevision || verdict.notes) && (
        <CriticLine verdict={verdict} />
      )}

      <TelemetryFooter decision={decision} />

      <button className="mt-2 flex items-center gap-1 text-[11px] text-cyan-400/60 hover:text-cyan-400 group-hover:opacity-100 opacity-0 transition-all">
        View full analysis
        <ChevronRight className="w-3 h-3" />
      </button>
    </motion.div>
  );
}

function ConfidencePill({ confidence }: { confidence: number }) {
  const pct = Math.round(confidence * 100);
  const tone =
    pct >= 75
      ? "text-emerald-300"
      : pct >= 50
        ? "text-yellow-400"
        : "text-gray-400";
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

function CriticLine({ verdict }: { verdict: CriticVerdict }) {
  const tone = verdict.demandsRevision
    ? "text-rose-300 border-rose-500/30 bg-rose-500/5"
    : "text-emerald-300/80 border-emerald-500/30 bg-emerald-500/5";
  return (
    <div
      className={`mt-3 flex items-start gap-2 px-2 py-1.5 rounded border ${tone} text-[10px] leading-relaxed`}
    >
      <ShieldAlert className="w-3 h-3 mt-0.5 shrink-0" />
      <span className="font-mono">
        <span className="opacity-70">
          Critic ({Math.round(verdict.confidence * 100)}%):
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
    <div className="mt-2 flex items-center gap-3 text-[10px] font-mono text-gray-600">
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
