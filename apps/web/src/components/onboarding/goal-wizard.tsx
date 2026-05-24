"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Sparkles, ShieldCheck, Coins } from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalButton,
} from "@aegis/ui";
import type {
  GoalHorizon,
  PortfolioObjective,
  RiskTolerance,
  RoutePreferences,
} from "@/types";
import { portfolioApi, analyticsApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

interface WizardState {
  step: 1 | 2 | 3;
  objective: PortfolioObjective;
  horizon: GoalHorizon;
  risk: RiskTolerance;
  submitting: boolean;
  error: string | null;
}

const STORAGE_KEY = "aegis.goal-wizard.draft";

const DEFAULT_ROUTE_PREFERENCES: RoutePreferences = {
  networks: ["ARC-TESTNET", "BASE-SEPOLIA"],
  networkWatchlist: ["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"],
  // The agent designs the target mix after this portfolio is created. Onboarding
  // seeds no token targets — only the executable networks the agent may use.
  tokens: ["USDC"],
  watchlist: ["BTC", "ETH", "SOL", "EURC"],
};

export function GoalWizard() {
  const router = useRouter();
  const addPortfolio = usePortfolioStore((s) => s.addPortfolio);

  const [state, setState] = useState<WizardState>(() => {
    if (typeof window !== "undefined") {
      try {
        const raw = window.sessionStorage.getItem(STORAGE_KEY);
        if (raw) {
          const parsed = JSON.parse(raw) as Partial<WizardState>;
          return {
            step: clampStep(parsed.step),
            objective: parsed.objective ?? "grow",
            horizon: parsed.horizon ?? "5y",
            risk: parsed.risk ?? "moderate",
            submitting: false,
            error: null,
          };
        }
      } catch {
        /* ignore */
      }
    }
    return {
      step: 1,
      objective: "grow",
      horizon: "5y",
      risk: "moderate",
      submitting: false,
      error: null,
    };
  });

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.sessionStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        step: state.step,
        objective: state.objective,
        horizon: state.horizon,
        risk: state.risk,
      }),
    );
  }, [state]);

  const submit = async () => {
    setState((s) => ({ ...s, submitting: true, error: null }));
    try {
      const goal: import("@/types").PortfolioGoal = {
        objective: state.objective,
        horizon: state.horizon,
        riskTolerance: state.risk,
        // The agent owns target weights — onboarding leaves them empty.
        targetAllocation: {},
        includeUsyc: false,
        includeEurc: false,
        routePreferences: DEFAULT_ROUTE_PREFERENCES,
        createdAt: new Date().toISOString(),
      };

      const portfolio = await portfolioApi.create({ allocations: [], goal });
      addPortfolio({ ...portfolio, allocations: [] });
      await analyticsApi.track("goal.completed", {
        portfolioId: portfolio.id,
        objective: state.objective,
        horizon: state.horizon,
        risk: state.risk,
      });

      // Navigate immediately — never block onboarding on the slow multi-model
      // allocator call. The dashboard kicks off the design on `?designing=1` and
      // opens Gate 1 when the proposal arrives (via the returned decision or the
      // SSE `agent.decision` event), with a retry affordance if it fails.
      window.sessionStorage.removeItem(STORAGE_KEY);
      router.push(`/dashboard/${portfolio.id}?designing=1`);
    } catch (e) {
      setState((s) => ({
        ...s,
        submitting: false,
        error: (e as Error).message,
      }));
    }
  };

  const go = (delta: 1 | -1) => {
    if (delta === 1 && state.step === 3) {
      void submit();
      return;
    }
    setState((s) => ({
      ...s,
      step: clampStep(s.step + delta),
    }));
  };

  return (
    <BrutalCard
      data-testid={`goal-wizard-step-${state.step}`}
      shadow={false}
      className="mx-auto w-full max-w-2xl"
    >
      <BrutalCardHeader className="block space-y-1">
        <div className="flex items-center justify-between gap-3">
          <span className="font-mono text-sm font-semibold text-text-hi">
            {stepTitle(state.step)}
          </span>
          <span className="shrink-0 font-mono text-[10px] uppercase tracking-widest text-text-mut">
            {state.step} / 3
          </span>
        </div>
        <p className="font-mono text-[11px] leading-relaxed text-text-mut">
          {stepHint(state.step)}
        </p>
      </BrutalCardHeader>
      <BrutalCardBody className="p-4 sm:p-5">
        <div
          className="mb-4 grid grid-cols-3 gap-2"
          aria-label="Portfolio setup progress"
        >
          {[1, 2, 3].map((step) => (
            <div
              key={step}
              className={`h-1.5 rounded-sharp ${
                step <= state.step ? "bg-accent-agent" : "bg-border-default"
              }`}
            />
          ))}
        </div>

        {state.step === 1 && (
          <ObjectiveStep state={state} setState={setState} />
        )}
        {state.step === 2 && <HorizonStep state={state} setState={setState} />}
        {state.step === 3 && <RiskStep state={state} setState={setState} />}

        {state.error && (
          <div className="mt-4 text-xs text-risk font-mono" role="alert">
            {state.error}
          </div>
        )}

        <div className="mt-6 grid gap-2 sm:grid-cols-2">
          <BrutalButton
            variant="ghost"
            className="min-h-11"
            disabled={state.step === 1 || state.submitting}
            onClick={() => go(-1)}
          >
            Back
          </BrutalButton>
          <BrutalButton
            variant="agent"
            className="min-h-11"
            disabled={state.submitting}
            onClick={() => go(1)}
          >
            {state.submitting
              ? "Designing allocation…"
              : state.step === 3
                ? "Let the agent design it"
                : "Next"}
          </BrutalButton>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function clampStep(step: number | undefined): WizardState["step"] {
  if (step === 2) return 2;
  if (step === 3) return 3;
  return 1;
}

function stepTitle(step: 1 | 2 | 3): string {
  return {
    1: "What is this money for?",
    2: "Investment horizon",
    3: "Risk tolerance",
  }[step];
}

function stepHint(step: 1 | 2 | 3): string {
  return {
    1: "Set the objective. The agent designs the allocation around it.",
    2: "Choose how long this money can stay invested.",
    3: "Pick the risk level the agent should respect.",
  }[step];
}

interface StepProps {
  state: WizardState;
  setState: React.Dispatch<React.SetStateAction<WizardState>>;
}

const OBJECTIVES: {
  value: PortfolioObjective;
  label: string;
  hint: string;
  icon: typeof Sparkles;
}[] = [
  {
    value: "grow",
    label: "Grow",
    hint: "Maximize long-term growth",
    icon: Sparkles,
  },
  {
    value: "preserve",
    label: "Preserve",
    hint: "Protect capital, limit drawdown",
    icon: ShieldCheck,
  },
  {
    value: "income",
    label: "Income",
    hint: "Steady yield from stable assets",
    icon: Coins,
  },
];

function ObjectiveStep({ state, setState }: StepProps) {
  return (
    <div className="space-y-2">
      {OBJECTIVES.map((o) => {
        const Icon = o.icon;
        const selected = state.objective === o.value;
        return (
          <button
            type="button"
            key={o.value}
            aria-pressed={selected}
            onClick={() => setState((s) => ({ ...s, objective: o.value }))}
            className={`flex min-h-12 w-full items-center gap-3 rounded-sharp border-brutal px-3 py-2 text-left font-mono text-sm transition-[box-shadow] ${
              selected
                ? "border-accent-agent bg-accent-agent/10 text-text-hi shadow-brutal-sm"
                : "border-border-default text-text-default hover:border-border-hi"
            }`}
          >
            <Icon
              className={`h-4 w-4 shrink-0 ${
                selected ? "text-accent-agent" : "text-text-mut"
              }`}
            />
            <span className="flex flex-col gap-0.5 sm:flex-row sm:items-center sm:gap-3">
              <span className="font-semibold">{o.label}</span>
              <span className="text-text-lo">{o.hint}</span>
            </span>
          </button>
        );
      })}
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
          type="button"
          key={h.value}
          aria-pressed={state.horizon === h.value}
          onClick={() => setState((s) => ({ ...s, horizon: h.value }))}
          className={`min-h-12 w-full rounded-sharp border-brutal px-3 py-2 text-left font-mono text-sm transition-[box-shadow] ${
            state.horizon === h.value
              ? "border-accent-agent bg-accent-agent/10 text-text-hi shadow-brutal-sm"
              : "border-border-default text-text-default hover:border-border-hi"
          }`}
        >
          <span className="flex flex-col gap-0.5 sm:flex-row sm:items-center sm:gap-3">
            <span className="font-semibold">{h.label}</span>
            <span className="text-text-lo">{h.hint}</span>
          </span>
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
          type="button"
          key={r.value}
          aria-pressed={state.risk === r.value}
          onClick={() => setState((s) => ({ ...s, risk: r.value }))}
          className={`min-h-12 w-full rounded-sharp border-brutal px-3 py-2 text-left font-mono text-sm transition-[box-shadow] ${
            state.risk === r.value
              ? "border-accent-agent bg-accent-agent/10 text-text-hi shadow-brutal-sm"
              : "border-border-default text-text-default hover:border-border-hi"
          }`}
        >
          <span className="flex flex-col gap-0.5 sm:flex-row sm:items-center sm:gap-3">
            <span className="font-semibold">{r.label}</span>
            <span className="text-text-lo">{r.hint}</span>
          </span>
        </button>
      ))}
    </div>
  );
}
