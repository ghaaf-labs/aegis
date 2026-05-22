import Link from "next/link";
import {
  ArrowRight,
  Brain,
  RadioTower,
  ShieldCheck,
  WalletCards,
} from "lucide-react";
import {
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { DEMO_BUNDLES } from "@/lib/explore-data";

export const metadata = {
  title: "Aegis · Explore demo portfolios",
  description:
    "Three curated Aegis portfolios across different risk profiles and regimes — see how the agent reasons before signing up.",
};

export default function ExploreIndex() {
  const bundles = Object.values(DEMO_BUNDLES);
  const totalDemoValue = bundles.reduce(
    (sum, { portfolio }) => sum + portfolio.totalValueUsd,
    0,
  );
  const decisionCount = bundles.reduce(
    (sum, { decisions }) => sum + decisions.length,
    0,
  );
  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div className="grid gap-5 border-brutal border-border-default bg-surface p-4 md:p-5 lg:grid-cols-[minmax(0,1fr)_520px]">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
              Explore demo portfolios
            </h1>
            <BrutalPill tone="agent">READ ONLY</BrutalPill>
          </div>
          <p className="mt-2 max-w-2xl text-sm text-text-lo">
            Inspect three curated portfolios across risk profiles. These demos
            show the reasoning path, target sleeves, model signal, and approval
            gates without opening a wallet session.
          </p>
          <div className="mt-4 grid gap-2 text-[11px] font-mono sm:grid-cols-3">
            <ExploreMetric
              icon={WalletCards}
              label="Demo value"
              value={formatCompactUsd(totalDemoValue)}
              tone="pnl"
            />
            <ExploreMetric
              icon={Brain}
              label="Agent notes"
              value={String(decisionCount)}
              tone="agent"
            />
            <ExploreMetric
              icon={ShieldCheck}
              label="Execution"
              value="blocked"
              tone="warn"
            />
          </div>
        </div>
        <ExploreRailSvg />
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {bundles.map(({ portfolio, decisions }) => (
          <Link
            key={portfolio.id}
            href={`/explore/${portfolio.id}`}
            className="block group"
          >
            <BrutalCard className="cursor-pointer transition-all group-hover:border-accent-agent/60 group-hover:translate-x-[-2px] group-hover:translate-y-[-2px] group-hover:shadow-brutal">
              <BrutalCardHeader>
                <div className="flex items-center justify-between w-full">
                  <span className="font-mono font-semibold text-text-hi">
                    {portfolio.name}
                  </span>
                  <BrutalPill tone="agent">
                    {portfolio.goal?.horizon}
                  </BrutalPill>
                </div>
              </BrutalCardHeader>
              <BrutalCardBody>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <p className="text-2xl font-mono font-semibold text-text-hi tabular-nums">
                      ${portfolio.totalValueUsd.toLocaleString()}
                    </p>
                    <p
                      className={`mt-1 text-sm font-mono ${
                        portfolio.totalPnlPct >= 0
                          ? "text-accent-pnl"
                          : "text-risk"
                      }`}
                    >
                      {portfolio.totalPnlPct >= 0 ? "+" : ""}
                      {portfolio.totalPnlPct.toFixed(1)}% all-time
                    </p>
                  </div>
                  <span className="border border-border-default bg-bg px-2 py-1 text-[10px] font-mono uppercase tracking-widest text-text-mut">
                    {portfolio.goal?.riskTolerance}
                  </span>
                </div>

                <p className="mt-4 min-h-10 text-xs leading-relaxed text-text-lo">
                  {portfolioThesis(portfolio.goal?.riskTolerance)}
                </p>

                <div className="mt-4 grid gap-2 text-[10px] font-mono">
                  <DemoFact
                    label="Target"
                    value={targetSummary(portfolio.goal?.targetAllocation)}
                  />
                  <DemoFact
                    label="Latest regime"
                    value={
                      decisions[0]?.regime?.replace("_", "-").toUpperCase() ??
                      "NONE"
                    }
                    tone="agent"
                  />
                  <DemoFact
                    label="Approval"
                    value="not executable in demo"
                    tone="warn"
                  />
                </div>

                <p className="mt-4 inline-flex items-center gap-1 text-[11px] text-accent-agent/70 group-hover:text-accent-agent font-mono">
                  Open reasoning trace
                  <ArrowRight className="h-3 w-3" />
                </p>
              </BrutalCardBody>
            </BrutalCard>
          </Link>
        ))}
      </div>

      <div className="flex flex-col gap-3 border border-accent-pnl/30 bg-accent-pnl/5 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <p className="text-sm font-mono font-semibold text-text-hi">
            Ready for a real wallet-backed portfolio?
          </p>
          <p className="mt-1 text-xs font-mono text-text-lo">
            Signup requires email verification, Circle W3S setup, and an
            explicit approval screen before deployment.
          </p>
        </div>
        <Link
          href="/signup"
          className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 px-4 py-2 bg-accent-pnl text-black font-mono font-semibold rounded-sharp border-brutal border-black shadow-brutal-sm hover:shadow-brutal"
        >
          Sign up to build your own
          <ArrowRight className="h-4 w-4" />
        </Link>
      </div>
    </div>
  );
}

function ExploreMetric({
  icon: Icon,
  label,
  value,
  tone,
}: {
  icon: typeof WalletCards;
  label: string;
  value: string;
  tone: "pnl" | "agent" | "warn";
}) {
  const toneClass =
    tone === "pnl"
      ? "text-accent-pnl"
      : tone === "agent"
        ? "text-accent-agent"
        : "text-warn";
  return (
    <div className="flex min-h-14 items-center gap-2 border border-border-default bg-bg px-3 py-2">
      <Icon className={`h-4 w-4 shrink-0 ${toneClass}`} />
      <div className="min-w-0">
        <p className="text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
        <p className={`mt-0.5 truncate font-semibold ${toneClass}`}>{value}</p>
      </div>
    </div>
  );
}

function DemoFact({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "agent" | "warn";
}) {
  const valueClass =
    tone === "agent"
      ? "text-accent-agent"
      : tone === "warn"
        ? "text-warn"
        : "text-text-hi";
  return (
    <div className="flex items-center justify-between gap-3 border border-border-default bg-bg px-2 py-1.5">
      <span className="uppercase tracking-widest text-text-mut">{label}</span>
      <span className={`truncate text-right ${valueClass}`}>{value}</span>
    </div>
  );
}

function ExploreRailSvg() {
  return (
    <svg
      viewBox="0 0 520 220"
      role="img"
      aria-label="Demo flow showing curated data, agent reasoning, review gate, and blocked execution"
      className="h-auto w-full border border-border-default bg-bg"
    >
      <defs>
        <pattern
          id="explore-grid"
          width="22"
          height="22"
          patternUnits="userSpaceOnUse"
        >
          <path d="M22 0H0V22" fill="none" stroke="#242424" strokeWidth="1" />
        </pattern>
      </defs>
      <rect width="520" height="220" fill="url(#explore-grid)" />
      <path
        d="M98 108H196H300H410"
        fill="none"
        stroke="#67e8f9"
        strokeDasharray="9 7"
        strokeWidth="4"
      >
        <animate
          attributeName="stroke-dashoffset"
          dur="2.2s"
          from="32"
          repeatCount="indefinite"
          to="0"
        />
      </path>
      <ExploreNode x={30} label="Demo" sublabel="curated" tone="agent" />
      <ExploreNode x={154} label="Agent" sublabel="reasoning" tone="agent" />
      <ExploreNode
        x={278}
        label="Review"
        sublabel="approval gate"
        tone="warn"
      />
      <ExploreNode x={398} label="Real" sublabel="signup only" tone="pnl" />
      <g transform="translate(34 166)">
        <RadioTower width="16" height="16" color="#67e8f9" />
        <text x="24" y="13" fill="#a3a3a3" fontFamily="monospace" fontSize="10">
          demo data never executes trades
        </text>
      </g>
    </svg>
  );
}

function ExploreNode({
  x,
  label,
  sublabel,
  tone,
}: {
  x: number;
  label: string;
  sublabel: string;
  tone: "pnl" | "agent" | "warn";
}) {
  const stroke =
    tone === "pnl" ? "#86efac" : tone === "agent" ? "#67e8f9" : "#f59e0b";
  return (
    <g transform={`translate(${x} 62)`}>
      <rect
        width="92"
        height="92"
        fill="#111111"
        stroke={stroke}
        strokeWidth="3"
      />
      <rect x="13" y="14" width="66" height="10" fill={stroke} />
      <text
        x="46"
        y="54"
        fill="#f5f5f5"
        fontFamily="monospace"
        fontSize="13"
        fontWeight="700"
        textAnchor="middle"
      >
        {label}
      </text>
      <text
        x="46"
        y="72"
        fill="#a3a3a3"
        fontFamily="monospace"
        fontSize="9"
        textAnchor="middle"
      >
        {sublabel}
      </text>
    </g>
  );
}

function targetSummary(
  targetAllocation: Record<string, number | undefined> | undefined,
): string {
  if (!targetAllocation) return "not set";
  return Object.entries(targetAllocation)
    .filter((entry): entry is [string, number] => (entry[1] ?? 0) > 0)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map(([symbol, pct]) => `${symbol} ${pct}%`)
    .join(" / ");
}

function portfolioThesis(risk: string | undefined) {
  if (risk === "conservative") {
    return "Yield and drawdown defense first; crypto beta is deliberately capped.";
  }
  if (risk === "aggressive") {
    return "Growth sleeve with higher drift tolerance before the agent trims risk.";
  }
  return "Treasury-style balance: stable yield, EUR exposure, and controlled BTC/ETH.";
}

function formatCompactUsd(value: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}
