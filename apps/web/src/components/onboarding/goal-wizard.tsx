"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { CheckCircle2, RotateCcw, SlidersHorizontal } from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalButton,
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
  attemptedAdvance: boolean;
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

const ALLOCATION_PRESETS: Array<{
  label: string;
  hint: string;
  allocation: Partial<Record<AssetSymbol, number>>;
}> = [
  {
    label: "Balanced core",
    hint: "Crypto growth with a small yield sleeve",
    allocation: DEFAULT_ALLOC,
  },
  {
    label: "Stable yield",
    hint: "Lower volatility, more USYC",
    allocation: { BTC: 20, ETH: 20, SOL: 0, USYC: 50, EURC: 10 },
  },
  {
    label: "Growth",
    hint: "Higher beta with no FX sleeve",
    allocation: { BTC: 55, ETH: 30, SOL: 15, USYC: 0, EURC: 0 },
  },
];

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
            step: parsed.step ?? 1,
            name: parsed.name ?? "",
            horizon: parsed.horizon ?? "5y",
            risk: parsed.risk ?? "moderate",
            allocation: parsed.allocation ?? DEFAULT_ALLOC,
            monthlyContribution: parsed.monthlyContribution ?? "",
            submitting: false,
            error: null,
            attemptedAdvance: false,
          };
        }
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
      attemptedAdvance: false,
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
  const disabledReason = nextDisabledReason(state, totalAlloc);

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
    if (delta === 1 && !canNext) {
      setState((s) => ({ ...s, attemptedAdvance: true }));
      return;
    }
    if (delta === 1 && state.step === 4 && canNext) {
      void submit();
      return;
    }
    setState((s) => ({
      ...s,
      attemptedAdvance: false,
      step: Math.max(1, Math.min(4, s.step + delta)) as WizardState["step"],
    }));
  };

  return (
    <BrutalCard
      data-testid={`goal-wizard-step-${state.step}`}
      shadow={false}
      className="mx-auto w-full max-w-lg"
    >
      <BrutalCardHeader className="block space-y-1">
        <div className="flex items-center justify-between gap-3">
          <span className="font-mono text-sm font-semibold text-text-hi">
            {stepTitle(state.step)}
          </span>
          <span className="shrink-0 font-mono text-[10px] uppercase tracking-widest text-text-mut">
            {state.step} / 4
          </span>
        </div>
        <p className="font-mono text-[11px] leading-relaxed text-text-mut">
          {stepHint(state.step)}
        </p>
      </BrutalCardHeader>
      <BrutalCardBody className="p-4 sm:p-5">
        <div
          className="mb-4 grid grid-cols-4 gap-2"
          aria-label="Portfolio setup progress"
        >
          {[1, 2, 3, 4].map((step) => (
            <div
              key={step}
              className={`h-1.5 rounded-sharp ${
                step <= state.step ? "bg-accent-agent" : "bg-border-default"
              }`}
            />
          ))}
        </div>

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
        {!canNext &&
          shouldShowNextHint(state, totalAlloc) &&
          disabledReason && (
            <div
              data-testid="goal-wizard-next-hint"
              className="mt-4 border border-warn/40 bg-warn/5 px-3 py-2 text-xs text-warn font-mono"
            >
              {disabledReason}
            </div>
          )}

        <div className="mt-6 grid grid-cols-2 gap-2">
          <BrutalButton
            variant="ghost"
            className="min-h-11"
            disabled={state.step === 1 || state.submitting}
            onClick={() => go(-1)}
          >
            Back
          </BrutalButton>
          <BrutalButton
            variant={state.step === 4 ? "pnl" : "agent"}
            className="min-h-11"
            disabled={state.submitting}
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

function stepHint(step: 1 | 2 | 3 | 4): string {
  return {
    1: "Give this target a name.",
    2: "Choose how long this money can stay invested.",
    3: "Pick the risk level the agent should respect.",
    4: "Set the target mix. You can change it later.",
  }[step];
}

function nextDisabledReason(state: WizardState, totalAlloc: number) {
  if (state.step === 1 && state.name.trim().length < 2) {
    return state.name.trim().length === 0
      ? "Enter a portfolio name to continue."
      : "Use at least 2 characters.";
  }
  if (state.step === 4 && Math.abs(totalAlloc - 100) >= 0.5) {
    const diff = Math.abs(100 - totalAlloc).toFixed(0);
    return totalAlloc < 100
      ? `Add ${diff}% more allocation, or use Normalize to finish.`
      : `Remove ${diff}% allocation, or use Normalize to finish.`;
  }
  return null;
}

function shouldShowNextHint(state: WizardState, totalAlloc: number) {
  const reason = nextDisabledReason(state, totalAlloc);
  if (!reason) return false;
  if (state.step === 1) {
    return state.attemptedAdvance || state.name.trim().length > 0;
  }
  return state.attemptedAdvance || state.step === 4;
}

interface StepProps {
  state: WizardState;
  setState: React.Dispatch<React.SetStateAction<WizardState>>;
}

function NameStep({ state, setState }: StepProps) {
  return (
    <div className="space-y-3">
      <label
        htmlFor="portfolio-name"
        className="block text-xs text-text-lo font-mono"
      >
        Portfolio name
      </label>
      <input
        id="portfolio-name"
        type="text"
        autoFocus
        value={state.name}
        onChange={(e) => setState((s) => ({ ...s, name: e.target.value }))}
        aria-describedby="portfolio-name-help"
        className="min-h-11 w-full rounded-sharp border-brutal border-border-default bg-bg px-3 py-2 font-mono text-base text-text-hi outline-none focus:border-border-hi sm:text-sm"
        placeholder="e.g. Retirement"
        maxLength={48}
      />
      <p id="portfolio-name-help" className="text-xs text-text-mut font-mono">
        Use a label you will recognize in Dashboard, approvals, and tax export.
      </p>
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

function AllocationStep({
  state,
  setState,
  totalAlloc,
}: StepProps & { totalAlloc: number }) {
  const allocationValid = Math.abs((totalAlloc ?? 0) - 100) < 0.5;

  const setAllocation = (allocation: Partial<Record<AssetSymbol, number>>) => {
    setState((s) => ({ ...s, allocation }));
  };

  return (
    <div className="space-y-4">
      <div className="grid gap-2">
        <div className="flex items-center gap-2 text-xs text-text-lo font-mono">
          <SlidersHorizontal className="h-3.5 w-3.5 text-accent-agent" />
          Pick a preset, then adjust the weights.
        </div>
        <div className="grid gap-2 sm:grid-cols-3">
          {ALLOCATION_PRESETS.map((preset) => (
            <button
              type="button"
              key={preset.label}
              onClick={() => setAllocation(preset.allocation)}
              className="min-h-[74px] rounded-sharp border border-border-default bg-bg px-3 py-2 text-left font-mono hover:border-accent-agent hover:bg-accent-agent/5"
            >
              <span className="block text-xs font-semibold text-text-hi">
                {preset.label}
              </span>
              <span className="mt-1 block text-[11px] leading-snug text-text-mut">
                {preset.hint}
              </span>
            </button>
          ))}
        </div>
      </div>

      <div
        className={`flex flex-wrap items-center justify-between gap-3 border px-3 py-2 font-mono ${
          allocationValid
            ? "border-accent-pnl/40 bg-accent-pnl/5"
            : "border-warn/40 bg-warn/5"
        }`}
      >
        <div className="flex items-center gap-2 text-xs">
          {allocationValid ? (
            <CheckCircle2 className="h-3.5 w-3.5 text-accent-pnl" />
          ) : (
            <SlidersHorizontal className="h-3.5 w-3.5 text-warn" />
          )}
          <span className="text-text-lo">
            Total target{" "}
            <span className={allocationValid ? "text-accent-pnl" : "text-warn"}>
              {totalAlloc ?? 0}%
            </span>{" "}
            / 100%
          </span>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => setAllocation(normalizeAllocation(state.allocation))}
            className="inline-flex min-h-8 items-center gap-1 rounded-sharp border border-border-default bg-bg px-2 text-[11px] text-text-lo hover:border-accent-agent hover:text-accent-agent"
          >
            <SlidersHorizontal className="h-3 w-3" />
            Normalize
          </button>
          <button
            type="button"
            onClick={() => setAllocation(DEFAULT_ALLOC)}
            className="inline-flex min-h-8 items-center gap-1 rounded-sharp border border-border-default bg-bg px-2 text-[11px] text-text-lo hover:border-border-hi hover:text-text-hi"
          >
            <RotateCcw className="h-3 w-3" />
            Reset
          </button>
        </div>
      </div>

      <div className="space-y-3">
        {ASSETS.map((a) => (
          <div key={a.symbol} className="grid grid-cols-[1fr_auto] gap-3">
            <label htmlFor={`alloc-${a.symbol}`} className="min-w-0 font-mono">
              <span className="block text-sm text-text-hi">{a.symbol}</span>
              <span className="block text-xs text-text-lo">{a.label}</span>
            </label>
            <div className="flex items-center gap-2">
              <input
                id={`alloc-${a.symbol}`}
                aria-label={`${a.symbol} target allocation`}
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
                className="min-h-10 w-20 rounded-sharp border-brutal border-border-default bg-bg px-2 py-1 text-right font-mono text-sm tabular-nums text-text-hi outline-none focus:border-border-hi"
              />
              <span className="text-text-mut text-xs">%</span>
            </div>
          </div>
        ))}
      </div>

      <div className="pt-3 border-t border-border-default">
        <label
          htmlFor="monthly-contribution"
          className="block text-xs text-text-lo font-mono mb-2"
        >
          Optional: monthly contribution (USD)
        </label>
        <input
          id="monthly-contribution"
          type="number"
          min={0}
          max={1_000_000}
          step={50}
          value={state.monthlyContribution}
          onChange={(e) =>
            setState((s) => ({ ...s, monthlyContribution: e.target.value }))
          }
          className="min-h-10 w-32 rounded-sharp border-brutal border-border-default bg-bg px-2 py-1 text-right font-mono text-sm tabular-nums text-text-hi outline-none focus:border-border-hi"
          placeholder="0"
        />
        <p className="mt-2 text-[11px] text-text-mut font-mono leading-relaxed">
          Used for planning and projections only; it does not schedule a
          payment.
        </p>
      </div>
    </div>
  );
}

function normalizeAllocation(allocation: Partial<Record<AssetSymbol, number>>) {
  const total = ASSETS.reduce((sum, asset) => {
    return sum + Math.max(0, allocation[asset.symbol] ?? 0);
  }, 0);

  if (total <= 0) return DEFAULT_ALLOC;

  const next: Partial<Record<AssetSymbol, number>> = {};
  let running = 0;
  ASSETS.forEach((asset, index) => {
    const raw = Math.max(0, allocation[asset.symbol] ?? 0);
    const value =
      index === ASSETS.length - 1
        ? Math.max(0, 100 - running)
        : Math.round((raw / total) * 100);
    next[asset.symbol] = value;
    running += value;
  });
  return next;
}
