"use client";

import type { Portfolio } from "@/types";
import { formatCurrency } from "@/lib/utils";

interface ValueFlowCardProps {
  portfolio: Portfolio | null;
  idleUsdc: number;
  idleEurc: number;
  investedUsd: number;
  walletCashStatus?: "idle" | "loading" | "ready" | "error";
}

export function ValueFlowCard({
  portfolio,
  idleUsdc,
  idleEurc,
  investedUsd,
  walletCashStatus = "ready",
}: ValueFlowCardProps) {
  const walletCashKnown = walletCashStatus === "ready";
  const walletCashUnavailable = walletCashStatus === "error";
  const walletCashLoading =
    walletCashStatus === "idle" || walletCashStatus === "loading";
  const hasIdleCash = walletCashKnown && (idleUsdc > 0.5 || idleEurc > 0.5);
  const hasInvested = investedUsd > 0.5;
  const targetCount =
    portfolio?.allocations?.filter((a) => a.targetWeight > 0).length ?? 0;
  const targetSet = targetCount > 0;

  return (
    <section
      aria-label="Portfolio value flow"
      className="relative overflow-hidden rounded-sharp border-brutal border-border-default bg-bg p-4 md:p-5"
    >
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-start">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
            Portfolio state map
          </p>
          <h2 className="mt-1 font-mono text-lg font-semibold text-text-hi">
            Cash is not invested until a review plan is approved
          </h2>
          <p className="mt-2 max-w-3xl font-mono text-xs leading-relaxed text-text-lo">
            Aegis keeps wallet cash, target weights, and confirmed positions as
            separate states. The dashboard shows idle Gateway balances on the
            left, proposed deployment in review, and only confirmed legs as
            invested portfolio value.
            {walletCashUnavailable &&
              " Gateway did not return a balance, so Aegis marks wallet cash as unknown instead of showing a false zero."}
          </p>
        </div>
        <div className="grid min-w-0 gap-2 text-[11px] font-mono sm:grid-cols-2 lg:w-[430px]">
          <FlowStat
            label="Wallet cash"
            value={
              walletCashUnavailable
                ? "unavailable"
                : walletCashKnown
                  ? `${formatCurrency(idleUsdc)} USDC${idleEurc > 0 ? ` + €${idleEurc.toFixed(2)} EURC` : ""}`
                  : "checking..."
            }
            active={hasIdleCash || walletCashUnavailable}
            tone={walletCashUnavailable ? "warn" : "pnl"}
          />
          <FlowStat
            label="Invested"
            value={formatCurrency(investedUsd)}
            active={hasInvested}
            tone="pnl"
          />
          <FlowStat
            label="Target"
            value={targetCount > 0 ? `${targetCount} sleeves` : "not set"}
            active={targetSet}
            tone="agent"
          />
          <FlowStat label="Approval" value="required" active tone="warn" />
        </div>
      </div>

      <div className="mt-4 grid gap-3 xl:grid-cols-[minmax(0,1fr)_330px] xl:items-stretch">
        <div className="min-w-0 overflow-hidden border border-border-default bg-surface">
          <ValueRailSvg
            hasIdleCash={hasIdleCash}
            hasInvested={hasInvested}
            targetSet={targetSet}
            walletCashUnavailable={walletCashUnavailable}
            walletCashLoading={walletCashLoading}
          />
        </div>
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-1">
          <FlowStage
            index="01"
            label="Gateway cash"
            value={
              walletCashUnavailable
                ? "unknown"
                : walletCashLoading
                  ? "checking"
                  : hasIdleCash
                    ? "ready"
                    : "none"
            }
            tone={
              walletCashUnavailable ? "warn" : hasIdleCash ? "pnl" : "muted"
            }
          />
          <FlowStage
            index="02"
            label="Target plan"
            value={targetSet ? `${targetCount} sleeves set` : "needs goal"}
            tone={targetSet ? "agent" : "muted"}
          />
          <FlowStage
            index="03"
            label="Approval"
            value="required before execution"
            tone="warn"
          />
          <FlowStage
            index="04"
            label="Portfolio value"
            value={hasInvested ? "confirmed positions" : "not invested yet"}
            tone={hasInvested ? "pnl" : "muted"}
          />
        </div>
      </div>
    </section>
  );
}

function FlowStat({
  label,
  value,
  active,
  tone,
}: {
  label: string;
  value: string;
  active: boolean;
  tone: "pnl" | "agent" | "warn";
}) {
  const toneClass =
    tone === "pnl"
      ? "text-accent-pnl"
      : tone === "agent"
        ? "text-accent-agent"
        : "text-warn";
  return (
    <div
      className={`border px-3 py-2 ${
        active
          ? "border-border-default bg-surface"
          : "border-border-default bg-raised/50"
      }`}
    >
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={`mt-1 truncate font-semibold tabular-nums ${
          active ? toneClass : "text-text-mut"
        }`}
      >
        {value}
      </p>
    </div>
  );
}

function FlowStage({
  index,
  label,
  value,
  tone,
}: {
  index: string;
  label: string;
  value: string;
  tone: "pnl" | "agent" | "warn" | "muted";
}) {
  const toneClass =
    tone === "pnl"
      ? "border-accent-pnl/40 text-accent-pnl"
      : tone === "agent"
        ? "border-accent-agent/40 text-accent-agent"
        : tone === "warn"
          ? "border-warn/40 text-warn"
          : "border-border-default text-text-mut";
  return (
    <div className={`border bg-raised px-3 py-2 font-mono ${toneClass}`}>
      <div className="flex items-center justify-between gap-3">
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
        <span className="text-[10px] tabular-nums">{index}</span>
      </div>
      <p className="mt-1 text-xs font-semibold text-text-hi">{value}</p>
    </div>
  );
}

function ValueRailSvg({
  hasIdleCash,
  hasInvested,
  targetSet,
  walletCashUnavailable,
  walletCashLoading,
}: {
  hasIdleCash: boolean;
  hasInvested: boolean;
  targetSet: boolean;
  walletCashUnavailable: boolean;
  walletCashLoading: boolean;
}) {
  const cashTone = walletCashUnavailable
    ? "warn"
    : hasIdleCash
      ? "pnl"
      : walletCashLoading
        ? "agent"
        : "muted";
  return (
    <svg
      role="img"
      aria-label="Wallet cash, target plan, approval, and invested position flow"
      viewBox="0 0 760 260"
      className="block h-auto w-full"
      preserveAspectRatio="xMidYMid meet"
    >
      <defs>
        <pattern
          id="value-rail-grid"
          width="20"
          height="20"
          patternUnits="userSpaceOnUse"
        >
          <path d="M20 0H0V20" fill="none" stroke="#202020" strokeWidth="1" />
        </pattern>
        <linearGradient id="value-rail-fade" x1="0" x2="1" y1="0" y2="0">
          <stop offset="0" stopColor="#050505" />
          <stop offset="0.48" stopColor="#111111" />
          <stop offset="1" stopColor="#050505" />
        </linearGradient>
      </defs>
      <rect width="760" height="260" fill="url(#value-rail-fade)" />
      <rect
        width="760"
        height="260"
        fill="url(#value-rail-grid)"
        opacity="0.72"
      />
      <text
        x="24"
        y="31"
        fill="#8A8A8A"
        fontFamily="monospace"
        fontSize="10"
        letterSpacing="2"
      >
        CAPITAL ROUTING MAP
      </text>
      <text x="24" y="52" fill="#E8E8E8" fontFamily="monospace" fontSize="13">
        cash becomes portfolio value only after review and execution
      </text>

      <path
        d="M86 116H674"
        fill="none"
        stroke="#2A2A2A"
        strokeWidth="8"
        strokeLinecap="square"
      />
      <RailSegment x1={86} x2={232} active={hasIdleCash} tone={cashTone} />
      <RailSegment x1={232} x2={378} active={targetSet} tone="agent" />
      <RailSegment x1={378} x2={524} active tone="warn" />
      <RailSegment x1={524} x2={674} active={hasInvested} tone="pnl" />

      <path
        d="M86 194H674"
        fill="none"
        stroke="#1E1E1E"
        strokeWidth="2"
        strokeLinecap="square"
      />
      <text x="24" y="118" fill="#8A8A8A" fontFamily="monospace" fontSize="10">
        wallet lane
      </text>
      <text x="24" y="197" fill="#8A8A8A" fontFamily="monospace" fontSize="10">
        value lane
      </text>

      <CapitalNode
        x={30}
        y={78}
        width={112}
        label="Wallet"
        sublabel={
          walletCashUnavailable
            ? "unknown"
            : walletCashLoading
              ? "checking"
              : hasIdleCash
                ? "cash ready"
                : "no cash"
        }
        tone={cashTone}
        active={hasIdleCash || walletCashUnavailable || walletCashLoading}
        glyph="W"
      />
      <CapitalNode
        x={176}
        y={78}
        width={112}
        label="Target"
        sublabel={targetSet ? "set" : "missing"}
        tone="agent"
        active={targetSet}
        glyph="T"
      />
      <CapitalNode
        x={322}
        y={78}
        width={112}
        label="Review"
        sublabel="you approve"
        tone="warn"
        active
        glyph="R"
      />
      <CapitalNode
        x={468}
        y={78}
        width={112}
        label="Execute"
        sublabel="Arc + Base"
        tone="agent"
        active
        glyph="X"
      />
      <CapitalNode
        x={614}
        y={78}
        width={112}
        label="Invested"
        sublabel={hasInvested ? "confirmed" : "pending"}
        tone="pnl"
        active={hasInvested}
        glyph="$"
      />

      <rect
        x="322"
        y="174"
        width="112"
        height="42"
        fill="#111111"
        stroke={colorForTone("warn", true)}
        strokeWidth="1.5"
      />
      <text x="334" y="193" fill="#FFB800" fontFamily="monospace" fontSize="11">
        human gate
      </text>
      <text x="334" y="209" fill="#8A8A8A" fontFamily="monospace" fontSize="9">
        no auto-trade
      </text>
      <rect
        x="614"
        y="174"
        width="112"
        height="42"
        fill="#111111"
        stroke={colorForTone(hasInvested ? "pnl" : "muted", true)}
        strokeWidth="1.5"
      />
      <text
        x="626"
        y="193"
        fill={hasInvested ? "#00FF88" : "#8A8A8A"}
        fontFamily="monospace"
        fontSize="11"
      >
        value counted
      </text>
      <text x="626" y="209" fill="#8A8A8A" fontFamily="monospace" fontSize="9">
        after receipts
      </text>
    </svg>
  );
}

function RailSegment({
  x1,
  x2,
  active,
  tone,
}: {
  x1: number;
  x2: number;
  active: boolean;
  tone: "pnl" | "agent" | "warn" | "muted";
}) {
  const stroke = colorForTone(tone, active);
  return (
    <path
      d={`M${x1} 107H${x2}`}
      fill="none"
      stroke={stroke}
      strokeDasharray="12 8"
      strokeLinecap="square"
      strokeWidth="4"
    >
      {active && <AnimateFlow />}
    </path>
  );
}

function CapitalNode({
  x,
  y,
  width,
  label,
  sublabel,
  tone,
  active,
  glyph,
}: {
  x: number;
  y: number;
  width: number;
  label: string;
  sublabel: string;
  tone: "pnl" | "agent" | "warn" | "muted";
  active: boolean;
  glyph: string;
}) {
  const stroke = colorForTone(tone, active);

  return (
    <g transform={`translate(${x} ${y})`}>
      <rect
        width={width}
        height="104"
        fill={active ? "#111111" : "#141414"}
        stroke={stroke}
        strokeWidth="2"
      />
      <rect x="13" y="13" width="34" height="34" fill={stroke} />
      <text
        x="30"
        y="36"
        fill="#050505"
        fontFamily="monospace"
        fontSize="16"
        fontWeight="700"
        textAnchor="middle"
      >
        {glyph}
      </text>
      <text
        x="13"
        y="68"
        fill="#FFFFFF"
        fontFamily="monospace"
        fontSize="12"
        fontWeight="700"
      >
        {label}
      </text>
      <text x="13" y="86" fill="#8A8A8A" fontFamily="monospace" fontSize="10">
        {sublabel}
      </text>
      <rect
        x="13"
        y="94"
        width={width - 26}
        height="4"
        fill={stroke}
        opacity="0.55"
      />
      {active && (
        <rect x="13" y="94" width="18" height="4" fill="#FFFFFF" opacity="0.65">
          <animate
            attributeName="x"
            dur="2.2s"
            repeatCount="indefinite"
            values={`13;${width - 31};13`}
          />
        </rect>
      )}
    </g>
  );
}

function colorForTone(
  tone: "pnl" | "agent" | "warn" | "muted",
  active: boolean,
) {
  if (!active && tone !== "warn") return "#5A5A5A";
  if (tone === "pnl") return "#00FF88";
  if (tone === "agent") return "#00E0FF";
  if (tone === "warn") return "#FFB800";
  return "#5A5A5A";
}

function AnimateFlow() {
  return (
    <animate
      attributeName="stroke-dashoffset"
      dur="2.4s"
      from="36"
      repeatCount="indefinite"
      to="0"
    />
  );
}
