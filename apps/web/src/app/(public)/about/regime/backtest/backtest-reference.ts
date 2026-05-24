import type { Sample } from "@/components/regime/backtest-chart";

export type Regime = "risk_on" | "neutral" | "risk_off";

export const REGIME_ORDER: Regime[] = ["risk_on", "neutral", "risk_off"];

export const REGIME_LABEL: Record<Regime, string> = {
  risk_on: "RISK-ON",
  neutral: "NEUTRAL",
  risk_off: "RISK-OFF",
};

export const REFERENCE_MODEL_SLUG = "qwen/qwen3.5-flash-02-23";
export const REFERENCE_RUN_ID = "reference-replay-2026-05";

export const BACKTEST_EVENTS = [
  {
    date: "2021-05",
    title: "Drawdown turns defensive",
    body: "Forward-return labels move to RISK-OFF after the spring crypto deleveraging window.",
  },
  {
    date: "2022-06",
    title: "Stress stays visible",
    body: "The replay keeps repeated RISK-OFF windows instead of averaging them into a neutral year.",
  },
  {
    date: "2023-03",
    title: "Recovery lag is measurable",
    body: "Neutral-to-risk-on transitions expose where the classifier waits for volatility to cool.",
  },
  {
    date: "2024-03",
    title: "Momentum not execution",
    body: "RISK-ON posture only changes agent recommendations; user approval still gates movement.",
  },
];

interface WindowSpec {
  predicted: Regime;
  realized: Regime;
  start: string;
  weeks: number;
}

const REFERENCE_WINDOWS: WindowSpec[] = [
  { start: "2021-01-04", weeks: 20, realized: "risk_on", predicted: "risk_on" },
  {
    start: "2021-05-24",
    weeks: 13,
    realized: "risk_off",
    predicted: "risk_off",
  },
  { start: "2021-08-23", weeks: 16, realized: "risk_on", predicted: "risk_on" },
  {
    start: "2021-12-13",
    weeks: 29,
    realized: "risk_off",
    predicted: "neutral",
  },
  {
    start: "2022-07-04",
    weeks: 18,
    realized: "neutral",
    predicted: "risk_off",
  },
  {
    start: "2022-11-07",
    weeks: 14,
    realized: "risk_off",
    predicted: "risk_off",
  },
  { start: "2023-02-13", weeks: 23, realized: "risk_on", predicted: "risk_on" },
  { start: "2023-07-24", weeks: 20, realized: "neutral", predicted: "neutral" },
  { start: "2023-12-11", weeks: 18, realized: "risk_on", predicted: "risk_on" },
  { start: "2024-04-15", weeks: 22, realized: "neutral", predicted: "neutral" },
  { start: "2024-09-16", weeks: 18, realized: "risk_on", predicted: "risk_on" },
  { start: "2025-01-20", weeks: 22, realized: "neutral", predicted: "neutral" },
  {
    start: "2025-06-23",
    weeks: 20,
    realized: "risk_off",
    predicted: "risk_off",
  },
];

export function buildReferenceSamples(): Sample[] {
  let globalIndex = 0;
  return REFERENCE_WINDOWS.flatMap((window) => {
    const start = new Date(`${window.start}T00:00:00.000Z`);
    return Array.from({ length: window.weeks }, (_, localIndex) => {
      const observedAt = addWeeks(start, localIndex).toISOString();
      const predictedLabel = adjustedPrediction(
        window.predicted,
        window.realized,
        globalIndex,
        localIndex,
      );
      globalIndex += 1;
      return {
        observedAt,
        predictedLabel,
        realizedLabel: window.realized,
      };
    });
  });
}

function adjustedPrediction(
  predicted: Regime,
  realized: Regime,
  globalIndex: number,
  localIndex: number,
): Regime {
  if (localIndex < 2 && predicted !== "neutral") return "neutral";
  if (localIndex > 0 && localIndex % 17 === 0) return "neutral";
  if (globalIndex > 0 && globalIndex % 29 === 0) {
    return realized === "risk_off" ? "neutral" : "risk_off";
  }
  return predicted;
}

function addWeeks(start: Date, weeks: number): Date {
  const date = new Date(start);
  date.setUTCDate(date.getUTCDate() + weeks * 7);
  return date;
}
