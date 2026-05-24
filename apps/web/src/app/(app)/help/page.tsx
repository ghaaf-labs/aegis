import type { Metadata } from "next";
import Link from "next/link";
import {
  ArrowRight,
  CheckCircle2,
  CircleHelp,
  FileText,
  History,
  LifeBuoy,
  ShieldAlert,
  type LucideIcon,
  Wallet,
} from "lucide-react";
import { BrutalPill, ProvenanceLine } from "@aegis/ui";
import {
  QUICK_PATHS,
  STATUS_ROWS,
  SUPPORT_ROWS,
  type HelpTone,
  type QuickPathItem,
  type StatusRowItem,
} from "./help-page-data";
import { HelpItemGrid } from "./help-item-grid";
import { pageMetadata } from "@/lib/seo";
import { cn } from "@/lib/utils";

export const metadata: Metadata = pageMetadata({
  title: "Help — Aegis",
  description:
    "Plain-English help for wallet cash, approvals, agent decisions, tax exports, and support.",
  path: "/help",
});

export default function HelpPage() {
  return (
    <div className="mx-auto max-w-[1400px] space-y-5 md:space-y-6">
      <HelpHero />
      <HelpItemGrid />
      <OperationalGuide />
      <SupportBoundaries />
    </div>
  );
}

function HelpHero() {
  return (
    <section className="border-brutal border-border-default bg-surface">
      <div className="grid gap-5 border-b border-border-default px-4 py-5 md:grid-cols-[minmax(0,1fr)_minmax(280px,420px)] md:px-5">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <CircleHelp className="h-4 w-4 text-accent-agent" />
            <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
              Product guide
            </p>
            <BrutalPill tone="agent">SIGNED-OUT SAFE</BrutalPill>
          </div>
          <h1 className="mt-3 font-mono text-3xl font-semibold text-text-hi md:text-4xl">
            Help
          </h1>
          <p className="mt-3 max-w-3xl text-sm leading-relaxed text-text-lo">
            Find the exact surface that explains wallet cash, review approval,
            failed execution traces, public diary privacy, and tax exports.
            Public answers stay visible; account-specific pages ask you to sign
            in before opening data.
          </p>
          <div className="mt-3">
            <ProvenanceLine
              source="Aegis product guide"
              freshness="current routes"
            />
          </div>
        </div>

        <div className="grid gap-2 font-mono text-xs">
          {QUICK_PATHS.map((path) => (
            <QuickPath key={path.href} item={path} />
          ))}
        </div>
      </div>

      <div className="grid grid-cols-2 border-b border-border-default lg:grid-cols-4">
        <Metric
          icon={Wallet}
          label="Cash"
          value="Wallets first"
          detail="uninvested balance"
          tone="pnl"
        />
        <Metric
          icon={CheckCircle2}
          label="Moves"
          value="Approval first"
          detail="nothing moves silently"
          tone="agent"
        />
        <Metric
          icon={History}
          label="Trace"
          value="Transactions"
          detail="planned, running, failed"
          tone="agent"
        />
        <Metric
          icon={FileText}
          label="Reports"
          value="Tax Center"
          detail="settled activity only"
        />
      </div>
    </section>
  );
}

function OperationalGuide() {
  return (
    <section className="grid gap-5 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
      <Panel
        icon={Wallet}
        title="Where money appears"
        detail="wallet, review, portfolio"
      >
        <p className="text-sm leading-relaxed text-text-lo">
          One dollar can be in only one operational state at a time. Before
          approval it is wallet cash. During a review it is proposed movement.
          After successful execution it becomes a portfolio position or
          remaining reserve cash.
        </p>
        <div className="mt-4 grid gap-2 font-mono text-[11px] sm:grid-cols-3">
          <FlowStep
            step="1"
            label="Wallet"
            value="Cash before approval"
            tone="pnl"
          />
          <FlowStep
            step="2"
            label="Review"
            value="Plan and approval"
            tone="agent"
          />
          <FlowStep
            step="3"
            label="Portfolio"
            value="Positions after execution"
            tone="pnl"
          />
        </div>
      </Panel>

      <Panel
        icon={ShieldAlert}
        title="Plan status language"
        detail="what to do next"
      >
        <div className="grid gap-2">
          {STATUS_ROWS.map((row) => (
            <StatusRow key={row.status} row={row} />
          ))}
        </div>
      </Panel>
    </section>
  );
}

function SupportBoundaries() {
  return (
    <section className="border-brutal border-border-default bg-surface">
      <div className="grid gap-4 border-b border-border-default px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center md:px-5">
        <div>
          <div className="flex items-center gap-2">
            <LifeBuoy className="h-4 w-4 text-accent-agent" />
            <h2 className="font-mono text-lg font-semibold text-text-hi">
              Support boundaries
            </h2>
          </div>
          <p className="mt-2 max-w-3xl text-sm leading-relaxed text-text-lo">
            The fastest report is specific: include what page you were on, the
            review or trace ID, and the visible status. Do not send secrets. Use
            the policy page for fee and refund boundaries.
          </p>
        </div>
        <LinkButton href="/policy#refunds">Open refund policy</LinkButton>
      </div>
      <div className="grid gap-0 md:grid-cols-3">
        {SUPPORT_ROWS.map(([label, value]) => (
          <div
            key={label}
            className="border-b border-r border-border-default px-4 py-4 last:border-r-0 md:border-b-0"
          >
            <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
              {label}
            </p>
            <p className="mt-2 text-sm leading-relaxed text-text-lo">{value}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function QuickPath({ item }: { item: QuickPathItem }) {
  const Icon = item.icon;
  return (
    <Link
      href={item.href}
      className="grid min-h-14 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border border-border-default bg-bg px-3 py-2 hover:border-accent-agent hover:bg-raised"
    >
      <Icon className={cn("h-4 w-4", toneClass(item.tone))} />
      <span className="min-w-0">
        <span className="block font-semibold text-text-hi">{item.label}</span>
        <span className="mt-0.5 block truncate text-[10px] uppercase tracking-widest text-text-mut">
          {item.value}
        </span>
      </span>
      <ArrowRight className="h-3.5 w-3.5 text-text-mut" />
    </Link>
  );
}

function Metric({
  detail,
  icon: Icon,
  label,
  tone = "default",
  value,
}: {
  detail: string;
  icon: LucideIcon;
  label: string;
  tone?: "default" | "pnl" | "agent" | "warn" | "risk";
  value: string;
}) {
  return (
    <div className="min-h-24 border-r border-border-default px-4 py-4 last:border-r-0 odd:border-b even:border-b md:px-5 lg:border-b-0">
      <div className="flex items-center gap-2">
        <Icon className={cn("h-4 w-4 shrink-0", toneClass(tone))} />
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p
        className={cn("mt-3 font-mono text-xl font-semibold", toneClass(tone))}
      >
        {value}
      </p>
      <p className="mt-1 font-mono text-[10px] text-text-mut">{detail}</p>
    </div>
  );
}

function Panel({
  children,
  detail,
  icon: Icon,
  title,
}: {
  children: React.ReactNode;
  detail: string;
  icon: LucideIcon;
  title: string;
}) {
  return (
    <section className="border-brutal border-border-default bg-surface">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border-default px-4 py-3 md:px-5">
        <div className="flex items-center gap-2">
          <Icon className="h-4 w-4 text-accent-agent" />
          <h2 className="font-mono text-lg font-semibold text-text-hi">
            {title}
          </h2>
        </div>
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {detail}
        </p>
      </div>
      <div className="p-4 md:p-5">{children}</div>
    </section>
  );
}

function FlowStep({
  label,
  step,
  tone,
  value,
}: {
  label: string;
  step: string;
  tone: "pnl" | "agent";
  value: string;
}) {
  return (
    <div
      className={cn(
        "border px-3 py-2",
        tone === "pnl"
          ? "border-accent-pnl/35 bg-accent-pnl/5"
          : "border-accent-agent/35 bg-accent-agent/5",
      )}
    >
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "flex h-5 w-5 shrink-0 items-center justify-center border bg-bg font-mono text-[10px] font-semibold",
            toneClass(tone),
          )}
        >
          {step}
        </span>
        <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
          {label}
        </p>
      </div>
      <p className="mt-2 font-mono text-xs text-text-hi">{value}</p>
    </div>
  );
}

function StatusRow({ row }: { row: StatusRowItem }) {
  return (
    <div className="grid gap-2 border border-border-default bg-bg px-3 py-2 font-mono text-xs md:grid-cols-[110px_minmax(0,1fr)]">
      <div>
        <p className={cn("font-semibold", toneClass(row.tone))}>{row.status}</p>
      </div>
      <div>
        <p className="text-text-hi">{row.meaning}</p>
        <p className="mt-1 text-text-mut">{row.action}</p>
      </div>
    </div>
  );
}

function LinkButton({
  children,
  href,
}: {
  children: React.ReactNode;
  href: string;
}) {
  return (
    <Link
      href={href}
      className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 border border-accent-agent/40 bg-accent-agent/5 px-4 font-mono text-xs font-semibold text-accent-agent hover:border-accent-agent"
    >
      {children}
      <ArrowRight className="h-3.5 w-3.5" />
    </Link>
  );
}

function toneClass(tone: "default" | HelpTone) {
  if (tone === "pnl") return "text-accent-pnl";
  if (tone === "agent") return "text-accent-agent";
  if (tone === "warn") return "text-warn";
  if (tone === "risk") return "text-risk";
  return "text-text-hi";
}
