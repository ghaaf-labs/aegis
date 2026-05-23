import { Loader2, RotateCw } from "lucide-react";

type GatewayBalanceStatus = "idle" | "loading" | "ready" | "error";

export function WalletOperationalPanel({
  gatewayBalanceStatus,
  idleCashUsd,
  refreshingGateway,
  onRefreshGateway,
}: {
  gatewayBalanceStatus: GatewayBalanceStatus;
  idleCashUsd: number;
  refreshingGateway: boolean;
  onRefreshGateway: () => void;
}) {
  const gatewayLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const headline =
    gatewayBalanceStatus === "error"
      ? "Wallet ready. Cash balance needs retry."
      : gatewayLoading
        ? "Wallet ready. Checking available cash."
        : idleCashUsd > 0.01
          ? "Wallet ready. Cash is available."
          : "Wallet ready. No idle cash available.";
  const detail =
    gatewayBalanceStatus === "error"
      ? "Your funding address is still usable. Retry the balance check before acting on wallet cash."
      : gatewayLoading
        ? "Copy your address any time. Cash totals appear after the balance check finishes."
        : idleCashUsd > 0.01
          ? "This cash is not invested yet. Review a plan before anything moves."
          : "You can still fund this address. New USDC appears here before it is invested.";

  return (
    <section className="border-brutal border-border-default bg-surface p-4 font-mono shadow-brutal">
      <div>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-widest text-accent-agent">
              Wallet status
            </p>
            <h2 className="mt-1 text-base font-semibold text-text-hi">
              {headline}
            </h2>
          </div>
          <button
            type="button"
            onClick={onRefreshGateway}
            disabled={refreshingGateway || gatewayLoading}
            className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-3 text-xs text-text-lo hover:border-accent-agent/40 hover:text-accent-agent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {refreshingGateway || gatewayLoading ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RotateCw className="h-3.5 w-3.5" />
            )}
            {gatewayLoading ? "Checking balance" : "Refresh balance"}
          </button>
        </div>
        <p className="mt-3 text-xs leading-relaxed text-text-lo">{detail}</p>
      </div>
    </section>
  );
}
