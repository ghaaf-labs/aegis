import Link from "next/link";
import { ArrowRight, CircleHelp } from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { HelpItemGrid } from "./help-item-grid";

export default function HelpPage() {
  return (
    <div className="mx-auto max-w-[1400px] space-y-6">
      <div>
        <p className="text-[10px] font-mono uppercase tracking-widest text-accent-agent">
          Product guide
        </p>
        <h1 className="mt-1 flex items-center gap-2 text-2xl font-mono font-semibold tracking-tight text-text-hi">
          <CircleHelp className="h-5 w-5 text-accent-agent" />
          Help
        </h1>
        <p className="mt-1 max-w-2xl text-sm text-text-lo">
          Short answers for wallet cash, approvals, agent decisions, tax
          exports, and support.
        </p>
      </div>

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
              Cash waits in Wallets. Approved moves become invested positions on
              Dashboard and Portfolio. Transactions shows what is waiting,
              running, finished, or no longer usable.
            </p>
            <div className="grid gap-2 text-[11px] font-mono sm:grid-cols-3">
              <HelpFact label="Wallet" value="idle USDC / EURC" />
              <HelpFact label="Portfolio" value="invested value" />
              <HelpFact label="Approvals" value="blocked vs executable" />
            </div>
          </div>
          <HelpFlowSvg />
        </BrutalCardBody>
      </BrutalCard>

      <HelpItemGrid />

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">Support policy</span>
        </BrutalCardHeader>
        <BrutalCardBody>
          <p className="text-sm leading-relaxed text-text-lo">
            Aegis refunds protocol fees for agent-caused execution failures,
            never market losses. The full policy page explains pause controls,
            dispute handling, and refund boundaries.
          </p>
          <Link
            href="/policy"
            className="mt-3 inline-flex min-h-10 items-center gap-2 border border-accent-agent/40 bg-accent-agent/10 px-3 py-2 text-xs font-mono text-accent-agent hover:border-accent-agent"
          >
            Open policy
            <ArrowRight className="h-3 w-3" />
          </Link>
        </BrutalCardBody>
      </BrutalCard>
    </div>
  );
}

function HelpFact({ label, value }: { label: string; value: string }) {
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
      <MapNode x={28} y={52} label="Wallet" sublabel="idle cash" tone="money" />
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
          after OK
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
