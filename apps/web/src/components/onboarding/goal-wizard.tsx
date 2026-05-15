"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalButton,
  BrutalPill,
} from "@aegis/ui";
import type { GoalHorizon, RiskTolerance, AssetSymbol } from "@/types";
import { portfolioApi, analyticsApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

interface WizardState {
  step: 1 | 2 | 3 | 4;
  name: string;
  horizon: GoalHorizon;
  risk: RiskTolerance;
  allocation: Partial<Record<AssetSymbol, number>>;
  monthlyContribution: string;
  submitting: boolean;
  error: string | null;
}

const STORAGE_KEY = "aegis.goal-wizard.draft";

/** EURC always in the allocation universe (user moves from 0% upward). */
const ASSETS: { symbol: AssetSymbol; label: string }[] = [
  { symbol: "BTC", label: "Bitcoin" },
  { symbol: "ETH", label: "Ethereum" },
  { symbol: "SOL", label: "Solana" },
  { symbol: "USYC", label: "USYC (yield)" },
  { symbol: "EURC", label: "EURC (EUR sleeve)" },
];

const DEFAULT_ALLOC: Partial<Record<AssetSymbol, number>> = {
  BTC: 50,
  ETH: 30,
  SOL: 10,
  USYC: 10,
  EURC: 0,
};

export function GoalWizard() {
  const router = useRouter();
  const addPortfolio = usePortfolioStore((s) => s.addPortfolio);

  const [state, setState] = useState<WizardState>(() => {
    if (typeof window !== "undefined") {
      try {
        const raw = window.sessionStorage.getItem(STORAGE_KEY);
        if (raw) return { ...JSON.parse(raw), submitting: false, error: null };
      } catch {
        /* ignore */
      }
    }
    return {
      step: 1,
      name: "",
      horizon: "5y",
      risk: "moderate",
      allocation: DEFAULT_ALLOC,
      monthlyContribution: "",
      submitting: false,
      error: null,
    };
  });

  useEffect(() => {
    if (typeof window === "undefined") return;
    const rest = {
      step: state.step,
      name: state.name,
      horizon: state.horizon,
      risk: state.risk,
      allocation: state.allocation,
      monthlyContribution: state.monthlyContribution,
    };
    window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(rest));
  }, [state]);

  const totalAlloc = Object.values(state.allocation).reduce(
    (a: number, b) => a + (b ?? 0),
    0,
  );
  const allocValid = Math.abs(totalAlloc - 100) < 0.5;

  const canNext =
    (state.step === 1 && state.name.trim().length >= 2) ||
    (state.step === 2 && !!state.horizon) ||
    (state.step === 3 && !!state.risk) ||
    (state.step === 4 && allocValid);

  const submit = async () => {
    setState((s) => ({ ...s, submitting: true, error: null }));
    try {
      const monthly = state.monthlyContribution
        ? Number(state.monthlyContribution)
        : undefined;
      const goal: import("@/types").PortfolioGoal = {
        name: state.name.trim(),
        horizon: state.horizon,
        riskTolerance: state.risk,
        targetAllocation: state.allocation,
        includeUsyc: (state.allocation.USYC ?? 0) > 0,
        includeEurc: (state.allocation.EURC ?? 0) > 0,
        ...(monthly !== undefined ? { monthlyContributionUsd: monthly } : {}),
        createdAt: new Date().toISOString(),
      };
      const allocations = ASSETS.filter(
        (a) => (state.allocation[a.symbol] ?? 0) > 0,
      ).map((a) => ({
        symbol: a.symbol,
        quantity: 0,
        targetWeight: state.allocation[a.symbol] ?? 0,
      }));

      const portfolio = await portfolioApi.create({
        name: state.name.trim(),
        allocations,
        goal,
      });
      // POST /portfolios returns the Portfolio row only; allocations live in
      // a separate table. The dashboard reads `.allocations.length`, so the
      // wizard merges the just-submitted weights into the store entry to
      // avoid an empty-state crash before the next GET hydrates real data.
      addPortfolio({
        ...portfolio,
        allocations: allocations.map((a) => ({
          assetId: a.symbol,
          symbol: a.symbol,
          quantity: a.quantity,
          targetWeight: a.targetWeight,
          currentWeight: 0,
          valueUsd: 0,
        })),
      });
      await analyticsApi.track("goal.completed", {
        portfolioId: portfolio.id,
        horizon: state.horizon,
        risk: state.risk,
      });
      window.sessionStorage.removeItem(STORAGE_KEY);
      router.push(`/dashboard/${portfolio.id}`);
    } catch (e) {
      setState((s) => ({
        ...s,
        submitting: false,
        error: (e as Error).message,
      }));
    }
  };

  const go = (delta: 1 | -1) => {
    if (delta === 1 && state.step === 4 && canNext) {
      void submit();
      return;
    }
    setState((s) => ({
      ...s,
      step: Math.max(1, Math.min(4, s.step + delta)) as WizardState["step"],
    }));
  };

  return (
    <BrutalCard className="max-w-xl mx-auto">
      <BrutalCardHeader>
        <div className="flex items-center gap-3">
          <BrutalPill tone="agent">STEP {state.step} / 4</BrutalPill>
          <span className="text-sm text-text-default font-semibold">
            {stepTitle(state.step)}
          </span>
        </div>
      </BrutalCardHeader>
      <BrutalCardBody>
        {state.step === 1 && <NameStep state={state} setState={setState} />}
        {state.step === 2 && <HorizonStep state={state} setState={setState} />}
        {state.step === 3 && <RiskStep state={state} setState={setState} />}
        {state.step === 4 && (
          <AllocationStep
            state={state}
            setState={setState}
            totalAlloc={totalAlloc}
          />
        )}

        {state.error && (
          <div className="mt-4 text-xs text-risk font-mono">{state.error}</div>
        )}

        <div className="mt-6 flex items-center justify-between gap-3">
          <BrutalButton
            variant="ghost"
            disabled={state.step === 1 || state.submitting}
            onClick={() => go(-1)}
          >
            Back
          </BrutalButton>
          <BrutalButton
            variant={state.step === 4 ? "pnl" : "agent"}
            disabled={!canNext || state.submitting}
            onClick={() => go(1)}
          >
            {state.submitting
              ? "Creating…"
              : state.step === 4
                ? "Create portfolio"
                : "Next"}
          </BrutalButton>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function stepTitle(step: 1 | 2 | 3 | 4): string {
  return {
    1: "Name your portfolio",
    2: "Investment horizon",
    3: "Risk tolerance",
    4: "Target allocation",
  }[step];
}

interface StepProps {
  state: WizardState;
  setState: React.Dispatch<React.SetStateAction<WizardState>>;
}

function NameStep({ state, setState }: StepProps) {
  return (
    <div className="space-y-3">
      <label className="block text-xs text-text-lo font-mono">
        Give this portfolio a name (e.g. Retirement, Treasury, Speculative).
      </label>
      <input
        autoFocus
        value={state.name}
        onChange={(e) => setState((s) => ({ ...s, name: e.target.value }))}
        className="w-full px-3 py-2 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi outline-none"
        placeholder="Retirement"
        maxLength={48}
      />
    </div>
  );
}

const HORIZONS: { value: GoalHorizon; label: string; hint: string }[] = [
  { value: "1y", label: "1 year", hint: "Conservative footing" },
  { value: "3y", label: "3 years", hint: "Short horizon" },
  { value: "5y", label: "5 years", hint: "Standard mid-term" },
  { value: "10y", label: "10 years", hint: "Long horizon" },
  { value: "20y+", label: "20+ years", hint: "Generational" },
];

function HorizonStep({ state, setState }: StepProps) {
  return (
    <div className="space-y-2">
      {HORIZONS.map((h) => (
        <button
          key={h.value}
          onClick={() => setState((s) => ({ ...s, horizon: h.value }))}
          className={`w-full text-left px-3 py-2 border-brutal rounded-sharp font-mono text-sm transition-[box-shadow] ${
            state.horizon === h.value
              ? "border-accent-agent bg-accent-agent/10 text-text-hi shadow-brutal-sm"
              : "border-border-default text-text-default hover:border-border-hi"
          }`}
        >
          <span className="font-semibold">{h.label}</span>
          <span className="ml-3 text-text-lo">{h.hint}</span>
        </button>
      ))}
    </div>
  );
}

const RISKS: { value: RiskTolerance; label: string; hint: string }[] = [
  {
    value: "conservative",
    label: "Conservative",
    hint: "Capital preservation first",
  },
  { value: "moderate", label: "Moderate", hint: "Balanced growth + safety" },
  {
    value: "aggressive",
    label: "Aggressive",
    hint: "Max growth, ride volatility",
  },
];

function RiskStep({ state, setState }: StepProps) {
  return (
    <div className="space-y-2">
      {RISKS.map((r) => (
        <button
          key={r.value}
          onClick={() => setState((s) => ({ ...s, risk: r.value }))}
          className={`w-full text-left px-3 py-2 border-brutal rounded-sharp font-mono text-sm transition-[box-shadow] ${
            state.risk === r.value
              ? "border-accent-agent bg-accent-agent/10 text-text-hi shadow-brutal-sm"
              : "border-border-default text-text-default hover:border-border-hi"
          }`}
        >
          <span className="font-semibold">{r.label}</span>
          <span className="ml-3 text-text-lo">{r.hint}</span>
        </button>
      ))}
    </div>
  );
}

function AllocationStep({
  state,
  setState,
  totalAlloc,
}: StepProps & { totalAlloc: number }) {
  return (
    <div className="space-y-3">
      <div className="text-xs text-text-lo font-mono">
        Weights must sum to <span className="text-text-hi">100%</span> —
        currently
        <span
          className={`ml-1 ${
            Math.abs((totalAlloc ?? 0) - 100) < 0.5
              ? "text-accent-pnl"
              : "text-warn"
          }`}
        >
          {totalAlloc ?? 0}%
        </span>
      </div>
      {ASSETS.map((a) => (
        <div key={a.symbol} className="flex items-center gap-3">
          <span className="w-20 font-mono text-sm text-text-hi">
            {a.symbol}
          </span>
          <span className="flex-1 text-xs text-text-lo">{a.label}</span>
          <input
            type="number"
            min={0}
            max={100}
            step={5}
            value={state.allocation[a.symbol] ?? 0}
            onChange={(e) =>
              setState((s) => ({
                ...s,
                allocation: {
                  ...s.allocation,
                  [a.symbol]: Math.max(
                    0,
                    Math.min(100, Number(e.target.value) || 0),
                  ),
                },
              }))
            }
            className="w-20 px-2 py-1 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi text-right tabular-nums outline-none"
          />
          <span className="text-text-mut text-xs">%</span>
        </div>
      ))}

      <div className="pt-3 border-t border-border-default">
        <label className="block text-xs text-text-lo font-mono mb-2">
          Optional: monthly contribution (USD)
        </label>
        <input
          type="number"
          min={0}
          value={state.monthlyContribution}
          onChange={(e) =>
            setState((s) => ({ ...s, monthlyContribution: e.target.value }))
          }
          className="w-32 px-2 py-1 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi text-right tabular-nums outline-none"
          placeholder="0"
        />
      </div>
    </div>
  );
}
