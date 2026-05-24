"use client";

import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Brain,
  HandIcon,
  RefreshCw,
  Wifi,
  WifiOff,
  Wrench,
} from "lucide-react";
import {
  BrutalButton,
  BrutalCard as Card,
  BrutalCardBody as CardContent,
  BrutalCardHeader as CardHeader,
  BrutalCardTitle as CardTitle,
} from "@aegis/ui";
import { agentApi } from "@/lib/api";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";
import type { AgentAbstained, AgentToolInvoked } from "@/types";
import { DecisionList } from "./decision-log-list";

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
    <Card className="flex h-full min-h-[360px] flex-col">
      <CardHeader className="flex min-h-[52px] flex-row items-center justify-between pb-3">
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
          <BrutalButton
            variant="ghost"
            className="min-h-11 border-transparent px-2 text-text-mut hover:text-text-default"
            onClick={() => void handleRefresh()}
            disabled={refreshing || !portfolio}
            title="Refresh decisions"
            aria-label="Refresh decisions"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${refreshing ? "animate-spin" : ""}`}
            />
          </BrutalButton>
        </div>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col overflow-hidden p-0">
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

function LiveActivityStrip({
  toolInvocations,
  abstains,
}: {
  toolInvocations: AgentToolInvoked[];
  abstains: AgentAbstained[];
}) {
  return (
    <div className="border-b border-border-default bg-bg/70 px-5 py-3">
      <p className="mb-2 font-mono text-[10px] uppercase tracking-wider text-text-mut">
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
              className="flex items-center gap-2 font-mono text-[11px] text-text-lo"
            >
              <HandIcon className="h-3 w-3 shrink-0 text-warn" />
              <span className="opacity-75">Abstained</span>
              <span className="opacity-50">-</span>
              <span className="truncate">{a.reason}</span>
              <span className="ml-auto shrink-0 text-[10px] opacity-50">
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
              className="flex items-center gap-2 font-mono text-[11px] text-text-lo"
            >
              <Wrench className="h-3 w-3 shrink-0 text-accent-agent" />
              <span className="opacity-75">{t.toolName}</span>
              <span className="opacity-50">-</span>
              <span className="truncate opacity-60">{t.resultPreview}</span>
              <span className="ml-auto shrink-0 text-[10px] opacity-50">
                {t.latencyMs}ms
              </span>
            </motion.div>
          ))}
        </AnimatePresence>
      </div>
    </div>
  );
}
