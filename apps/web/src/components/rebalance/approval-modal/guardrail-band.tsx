import type { RebalanceApprovalSafety } from "@/lib/api";
import type { AgentDecision } from "@/types";
import { approvalBlockLabel } from "./helpers";

export function GuardrailBand({
  approvalBlocked,
  approvalBlockCode,
  approvalBlockMessage,
  approvalSafety,
  portfolioId,
  totalLegs,
  decision,
}: {
  approvalBlocked: boolean;
  approvalBlockCode: string;
  approvalBlockMessage: string;
  approvalSafety?: RebalanceApprovalSafety | null;
  portfolioId?: string | null;
  totalLegs: number;
  decision?: AgentDecision | null;
}) {
  if (approvalBlocked) {
    return (
      <div className="mb-4 border-brutal border-warn/45 bg-warn/5 p-4 text-xs font-mono text-warn">
        <p className="text-[10px] uppercase tracking-wider">
          {approvalBlockLabel(approvalBlockCode)}
        </p>
        <p className="mt-1 leading-relaxed">{approvalBlockMessage}</p>
        {approvalSafety?.missingCapabilities?.length ? (
          <ul className="mt-3 flex flex-wrap gap-2">
            {approvalSafety.missingCapabilities.map((capability) => (
              <li
                key={capability.code}
                className="border border-warn/30 bg-black/20 px-2 py-1.5 text-[10px] uppercase tracking-wider"
              >
                {capability.label}
              </li>
            ))}
          </ul>
        ) : null}
        {approvalSafety ? (
          <BlockedRecoveryActions
            portfolioId={portfolioId ?? null}
            safety={approvalSafety}
          />
        ) : null}
      </div>
    );
  }

  if (totalLegs > 0) {
    return (
      <div className="mb-3 border border-accent-pnl/30 bg-accent-pnl/5 px-3 py-2 font-mono text-[11px] text-text-lo">
        <p className="text-[10px] uppercase tracking-wider text-accent-pnl">
          Guardrails passed
        </p>
        <p className="mt-1 leading-relaxed">
          Within the safety clamps — ≤60% single asset, stable reserve floor, $5
          minimum move, executable routes only
          {decision?.regime ? `, ${decision.regime} regime bands` : ""}. Nothing
          moves until you approve.
        </p>
      </div>
    );
  }

  return null;
}

export function ReviewFact({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "agent" | "warn";
}) {
  const toneClass =
    tone === "agent"
      ? "text-accent-agent"
      : tone === "warn"
        ? "text-warn"
        : "text-text-hi";
  return (
    <div className="border border-white/10 bg-black/30 px-3 py-2">
      <p className="text-[9px] uppercase tracking-wider text-text-mut">
        {label}
      </p>
      <p className={`mt-1 font-semibold ${toneClass}`}>{value}</p>
    </div>
  );
}

function BlockedRecoveryActions({
  portfolioId,
  safety,
}: {
  portfolioId: string | null;
  safety: RebalanceApprovalSafety;
}) {
  const dashboardHref = portfolioId
    ? `/dashboard/${portfolioId}`
    : "/dashboard";
  const actions =
    safety.code === "BALANCE_UNAVAILABLE"
      ? [
          {
            href: "/wallets",
            label: "Check wallet cash",
            primary: true,
          },
          {
            href: dashboardHref,
            label: "Build fresh review after balances recover",
            primary: false,
          },
        ]
      : safety.code === "EXECUTION_UNAVAILABLE"
        ? [
            {
              href: dashboardHref,
              label: "Change target mix",
              primary: true,
            },
            {
              href: "/transactions",
              label: "Back to ledger",
              primary: false,
            },
          ]
        : [
            {
              href: dashboardHref,
              label: "Build fresh review",
              primary: true,
            },
            {
              href: "/transactions",
              label: "Back to ledger",
              primary: false,
            },
          ];

  return (
    <div className="mt-3 flex flex-col gap-2 sm:flex-row">
      {actions.map((action) => (
        <a
          key={action.label}
          href={action.href}
          className={
            action.primary
              ? "inline-flex min-h-9 flex-1 items-center justify-center border border-warn/50 bg-warn/10 px-3 py-1.5 text-center text-[11px] font-semibold text-warn hover:bg-warn/15"
              : "inline-flex min-h-9 flex-1 items-center justify-center border border-border-default bg-black/20 px-3 py-1.5 text-center text-[11px] text-text-lo hover:border-border-hi hover:text-text-hi"
          }
        >
          {action.label}
        </a>
      ))}
    </div>
  );
}
