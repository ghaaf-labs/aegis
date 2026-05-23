"use client";

import type { ReactNode } from "react";
import { Cpu, ShieldCheck } from "lucide-react";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";

interface WalletNetworkRoute {
  blockchain: string;
  address: string;
}

interface NetworkTokenPanelProps {
  networks: WalletNetworkRoute[];
}

const NETWORKS = [
  {
    blockchain: "ARC-TESTNET",
    label: "Arc testnet",
    state: "Live",
    detail: "USDC cash route",
  },
  {
    blockchain: "BASE-SEPOLIA",
    label: "Base Sepolia",
    state: "Live",
    detail: "USDC cash route",
  },
  {
    blockchain: "ETH-SEPOLIA",
    label: "Ethereum Sepolia",
    state: "Next",
    detail: "Wallet route not enabled",
  },
  {
    blockchain: "ARB-SEPOLIA",
    label: "Arbitrum Sepolia",
    state: "Next",
    detail: "Wallet route not enabled",
  },
  {
    blockchain: "AVAX-FUJI",
    label: "Avalanche Fuji",
    state: "Next",
    detail: "Wallet route not enabled",
  },
] as const;

const TOKENS = [
  {
    symbol: "USDC",
    label: "Cash",
    state: "Usable",
    detail: "Funding source and reserve target",
  },
  {
    symbol: "BTC / ETH / SOL",
    label: "Market targets",
    state: "Blocked",
    detail: "Needs live swap routes before approval",
  },
  {
    symbol: "USYC",
    label: "Yield",
    state: "Blocked",
    detail: "Needs live yield route before approval",
  },
  {
    symbol: "EURC",
    label: "FX sleeve",
    state: "Blocked",
    detail: "Needs live FX route before approval",
  },
] as const;

export function NetworkTokenPanel({ networks }: NetworkTokenPanelProps) {
  const liveBlockchains = new Set(
    networks.map((network) => network.blockchain),
  );

  return (
    <BrutalCard>
      <BrutalCardHeader className="gap-3">
        <span className="flex min-w-0 items-center gap-2 text-sm font-mono text-text-hi">
          <Cpu className="h-4 w-4 shrink-0 text-accent-agent" />
          Networks & tokens
        </span>
        <span className="shrink-0 text-[10px] font-mono uppercase tracking-wider text-text-mut">
          Agent scope
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-4">
        <p className="max-w-3xl text-xs leading-relaxed text-text-lo">
          The agent can only use live routes. Other networks and tokens stay
          visible as future choices, but approvals remain locked until their
          route is ready.
        </p>

        <div className="grid gap-3 lg:grid-cols-[1.1fr_0.9fr]">
          <section aria-label="Network routes" className="space-y-2">
            <div className="flex items-center gap-2 text-[10px] font-mono uppercase tracking-wider text-text-mut">
              <ShieldCheck className="h-3.5 w-3.5 text-accent-pnl" />
              Network routes
            </div>
            <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {NETWORKS.map((network) => {
                const live = liveBlockchains.has(network.blockchain);
                return (
                  <div
                    key={network.blockchain}
                    className={`rounded-sharp border p-3 ${
                      live
                        ? "border-accent-pnl/40 bg-accent-pnl/5"
                        : "border-border-default bg-raised"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-mono text-text-hi">
                          {network.label}
                        </p>
                        <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                          {live
                            ? network.detail
                            : "Not enabled for this wallet"}
                        </p>
                      </div>
                      <StatusPill tone={live ? "live" : "muted"}>
                        {live ? network.state : "Off"}
                      </StatusPill>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          <section aria-label="Token routes" className="space-y-2">
            <div className="text-[10px] font-mono uppercase tracking-wider text-text-mut">
              Token routes
            </div>
            <div className="grid gap-2">
              {TOKENS.map((token) => (
                <div
                  key={token.symbol}
                  className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 rounded-sharp border border-border-default bg-raised p-3"
                >
                  <div className="min-w-0">
                    <p className="text-sm font-mono text-text-hi">
                      {token.symbol}
                    </p>
                    <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                      {token.label} · {token.detail}
                    </p>
                  </div>
                  <StatusPill tone={token.state === "Usable" ? "live" : "warn"}>
                    {token.state}
                  </StatusPill>
                </div>
              ))}
            </div>
          </section>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function StatusPill({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "live" | "warn" | "muted";
}) {
  return (
    <span
      className={`rounded-sharp border px-2 py-1 text-[10px] font-mono uppercase tracking-wider ${
        tone === "live"
          ? "border-accent-pnl/50 bg-accent-pnl/10 text-accent-pnl"
          : tone === "warn"
            ? "border-warn/50 bg-warn/10 text-warn"
            : "border-border-default bg-bg text-text-mut"
      }`}
    >
      {children}
    </span>
  );
}
