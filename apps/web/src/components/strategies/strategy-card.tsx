import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import type { StrategyPublic } from "@/lib/api";

interface Props {
  strategy: StrategyPublic;
  actionLabel: string;
  onAction?: () => void;
  disabled?: boolean;
}

const RISK_TONE = {
  low: "pnl",
  medium: "neutral",
  high: "warn",
} as const;

export function StrategyCard({
  strategy,
  actionLabel,
  onAction,
  disabled,
}: Props) {
  const entries = Object.entries(strategy.targetAllocation).sort(
    (a, b) => b[1] - a[1],
  );
  const horizonLabel =
    strategy.minHorizonMonths >= 60
      ? "5y+"
      : strategy.minHorizonMonths >= 36
        ? "3y+"
        : strategy.minHorizonMonths >= 12
          ? "1y+"
          : `${strategy.minHorizonMonths}mo`;

  return (
    <BrutalCard className="flex flex-col">
      <BrutalCardHeader>
        <span className="text-sm font-mono font-semibold text-text-hi">
          {strategy.name}
        </span>
        <div className="flex gap-1.5">
          <BrutalPill tone={RISK_TONE[strategy.riskBand]}>
            {strategy.riskBand}
          </BrutalPill>
          <BrutalPill tone="neutral">{horizonLabel}</BrutalPill>
        </div>
      </BrutalCardHeader>
      <BrutalCardBody className="flex-1 flex flex-col gap-4">
        <p className="text-xs text-text-lo leading-relaxed">
          {strategy.description}
        </p>
        <dl className="grid grid-cols-2 gap-1 text-[11px] font-mono">
          {entries.map(([sym, pct]) => (
            <div
              key={sym}
              className="flex items-baseline justify-between border border-border-default px-2 py-1 bg-raised rounded-sharp"
            >
              <span className="text-text-default">{sym}</span>
              <span className="text-text-hi tabular-nums">{pct}%</span>
            </div>
          ))}
        </dl>
        <button
          type="button"
          onClick={onAction}
          disabled={disabled || !onAction}
          className="mt-auto inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold border-brutal border-black rounded-sharp bg-accent-pnl text-black hover:shadow-brutal-sm transition-[box-shadow] active:translate-y-px disabled:opacity-60 disabled:hover:shadow-none"
        >
          {actionLabel}
        </button>
      </BrutalCardBody>
    </BrutalCard>
  );
}
