import type { Metadata } from "next";
import Link from "next/link";
import {
  ArrowRight,
  CheckCircle2,
  CircleHelp,
  History,
  Wallet,
} from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { HelpItemGrid } from "./help-item-grid";

export const metadata: Metadata = {
  title: "Aegis · Help",
  description:
    "Plain-English help for wallet cash, approvals, agent decisions, tax exports, and support.",
};

export default function HelpPage() {
  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <section className="grid gap-4 border-brutal border-border-default bg-surface p-4 md:p-5 lg:grid-cols-[minmax(0,1fr)_minmax(280px,360px)] lg:items-end">
        <div>
          <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
            Product guide
          </p>
          <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
            <CircleHelp className="h-5 w-5 text-accent-agent" />
            Help
          </h1>
          <p className="mt-2 max-w-2xl text-sm leading-relaxed text-text-lo">
            Short answers for wallet cash, approvals, agent decisions, tax
            exports, and support. Each answer links to the exact place to fix or
            inspect it.
          </p>
        </div>
        <div className="grid gap-2 font-mono text-[11px] sm:grid-cols-3 lg:grid-cols-1">
          <HeroFact icon={Wallet} label="Cash" value="Wallets first" />
          <HeroFact icon={CheckCircle2} label="Moves" value="Approval first" />
          <HeroFact icon={History} label="History" value="Transactions" />
        </div>
      </section>

      <HelpItemGrid />

      <BrutalCard>
        <BrutalCardBody className="grid gap-5 lg:grid-cols-[1fr_420px] lg:items-center">
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <BrutalPill tone="agent">Where things live</BrutalPill>
            </div>
            <h2 className="font-mono text-lg font-semibold text-text-hi">
              One dollar can be in only one place
            </h2>
            <p className="max-w-2xl text-sm leading-relaxed text-text-lo">
              Wallet cash stays in Wallets until you approve a move. Approved
              moves become positions on Dashboard and Portfolio. Transactions
              shows what is waiting, running, finished, or needs a fresh review.
            </p>
            <div className="grid gap-2 text-[11px] font-mono sm:grid-cols-3">
              <HelpFact
                step="1"
                label="Wallet"
                value="Cash before approval"
                tone="pnl"
              />
              <HelpFact
                step="2"
                label="Review"
                value="You approve the plan"
                tone="agent"
              />
              <HelpFact
                step="3"
                label="Portfolio"
                value="Positions after execution"
                tone="pnl"
              />
            </div>
          </div>
          <HelpFlowSvg />
        </BrutalCardBody>
      </BrutalCard>

      <BrutalCard>
        <BrutalCardHeader className="gap-3">
          <span className="text-sm font-mono text-text-hi">Support policy</span>
          <span className="shrink-0 border border-accent-agent/40 bg-accent-agent/5 px-2 py-1 font-mono text-[10px] uppercase tracking-widest text-accent-agent">
            Plain boundary
          </span>
        </BrutalCardHeader>
        <BrutalCardBody className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
          <div className="space-y-3">
            <p className="text-sm leading-relaxed text-text-lo">
              Aegis refunds protocol fees for agent-caused execution failures,
              never market losses. The full policy page explains pause controls,
              dispute handling, and refund boundaries.
            </p>
            <div className="grid gap-2 font-mono text-[11px] sm:grid-cols-2">
              <PolicyFact label="Covered" value="protocol fee failures" />
              <PolicyFact label="Not covered" value="market price movement" />
            </div>
          </div>
          <Link
            href="/policy#refunds"
            className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent hover:border-accent-agent"
          >
            Open refund policy
            <ArrowRight className="h-3 w-3" />
          </Link>
        </BrutalCardBody>
      </BrutalCard>
    </div>
  );
}

function HeroFact({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Wallet;
  label: string;
  value: string;
}) {
  return (
    <div className="grid min-h-12 grid-cols-[auto_minmax(0,1fr)] items-center gap-2 border border-border-default bg-bg px-3 py-2">
      <Icon className="h-4 w-4 text-accent-agent" />
      <div className="min-w-0">
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
        <p className="truncate font-semibold text-text-hi">{value}</p>
      </div>
    </div>
  );
}

function HelpFact({
  step,
  label,
  value,
  tone,
}: {
  step: string;
  label: string;
  value: string;
  tone: "pnl" | "agent";
}) {
  const toneClass =
    tone === "pnl"
      ? "border-accent-pnl/35 bg-accent-pnl/5 text-accent-pnl"
      : "border-accent-agent/35 bg-accent-agent/5 text-accent-agent";
  return (
    <div className={`border px-3 py-2 ${toneClass}`}>
      <div className="flex items-center gap-2">
        <span className="flex h-5 w-5 shrink-0 items-center justify-center border border-current bg-bg text-[10px] font-semibold">
          {step}
        </span>
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p className="mt-2 text-text-hi">{value}</p>
    </div>
  );
}

function PolicyFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-border-default bg-bg px-3 py-2">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className="mt-1 text-text-hi">{value}</p>
    </div>
  );
}

function HelpFlowSvg() {
  return (
    <svg
      viewBox="0 0 420 184"
      role="img"
      aria-label="Aegis value map from wallet cash to portfolio positions to approval history"
      className="h-auto w-full border border-border-default bg-bg"
    >
      <defs>
        <pattern
          id="help-map-grid"
          width="18"
          height="18"
          patternUnits="userSpaceOnUse"
        >
          <path d="M18 0H0V18" fill="none" stroke="#242424" strokeWidth="1" />
        </pattern>
      </defs>
      <rect width="420" height="184" fill="url(#help-map-grid)" />
      <path
        d="M92 88H166C190 88 196 124 220 124H292"
        fill="none"
        stroke="#67e8f9"
        strokeDasharray="8 8"
        strokeWidth="3"
      >
        <animate
          attributeName="stroke-dashoffset"
          dur="2.4s"
          from="32"
          repeatCount="indefinite"
          to="0"
        />
      </path>
      <path
        d="M92 88H158C184 88 190 58 216 58H292"
        fill="none"
        stroke="#86efac"
        strokeDasharray="7 9"
        strokeWidth="3"
      >
        <animate
          attributeName="stroke-dashoffset"
          dur="2.9s"
          from="32"
          repeatCount="indefinite"
          to="0"
        />
      </path>
      <MapNode x={28} y={52} label="Wallet" sublabel="cash" tone="money" />
      <MapNode
        x={292}
        y={22}
        label="Portfolio"
        sublabel="invested"
        tone="money"
      />
      <MapNode
        x={292}
        y={102}
        label="Transactions"
        sublabel="approval state"
        tone="agent"
      />
      <g transform="translate(22 150)">
        <rect
          width="376"
          height="26"
          fill="#0b0b0b"
          stroke="#2a2a2a"
          strokeWidth="1"
        />
        <text
          x="12"
          y="11"
          fill="#8a8a8a"
          fontFamily="monospace"
          fontSize="9"
          fontWeight="700"
        >
          cash now
        </text>
        <text x="12" y="22" fill="#8a8a8a" fontFamily="monospace" fontSize="9">
          after approval
        </text>
      </g>
    </svg>
  );
}

function MapNode({
  x,
  y,
  label,
  sublabel,
  tone,
}: {
  x: number;
  y: number;
  label: string;
  sublabel: string;
  tone: "agent" | "money";
}) {
  return (
    <g>
      <rect
        x={x}
        y={y}
        width="100"
        height="56"
        fill={tone === "agent" ? "#67e8f9" : "#86efac"}
        stroke={tone === "agent" ? "#67e8f9" : "#86efac"}
        strokeWidth="2"
      />
      <rect
        x={x + 8}
        y={y + 8}
        width="84"
        height="40"
        fill="#0b0b0b"
        opacity="0.12"
      />
      <text
        x={x + 50}
        y={y + 25}
        fill="#0b0b0b"
        fontFamily="monospace"
        fontSize="12"
        fontWeight="700"
        textAnchor="middle"
      >
        {label}
      </text>
      <text
        x={x + 50}
        y={y + 42}
        fill="#0b0b0b"
        fontFamily="monospace"
        fontSize="9"
        textAnchor="middle"
      >
        {sublabel}
      </text>
    </g>
  );
}
