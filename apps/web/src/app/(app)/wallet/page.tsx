"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  Check,
  CircleAlert,
  Copy,
  ExternalLink,
  Loader2,
  LogIn,
  RotateCw,
  UserPlus,
  Wallet as WalletIcon,
} from "lucide-react";
import Link from "next/link";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";
import { FaucetButton } from "@/components/wallet/faucet-button";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency } from "@/lib/utils";
import { gatewayApi, walletApi } from "@/lib/api";
import { explorerAddressUrl, type ExplorerChain } from "@/lib/explorers";

/**
 * Dedicated wallet view — per-chain USDC + EURC balances with copy +
 * explorer affordances. Before this page the only balance surface was a
 * tiny "GATEWAY $X" string in the header.
 */
export default function WalletPage() {
  const wallet = usePortfolioStore((s) => s.wallet);
  const unifiedUsdc = usePortfolioStore((s) => s.unifiedUsdc);
  const unifiedEurc = usePortfolioStore((s) => s.unifiedEurc);
  const perChainUsdc = usePortfolioStore((s) => s.perChainUsdc);
  const perChainEurc = usePortfolioStore((s) => s.perChainEurc);
  const gatewayBalanceStatus = usePortfolioStore((s) => s.gatewayBalanceStatus);
  const gatewayBalanceError = usePortfolioStore((s) => s.gatewayBalanceError);
  const snapshot = usePortfolioStore((s) => s.marketSnapshot);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setUnifiedUsdc = usePortfolioStore((s) => s.setUnifiedUsdc);
  const setUnifiedEurc = usePortfolioStore((s) => s.setUnifiedEurc);
  const setPerChain = usePortfolioStore((s) => s.setPerChain);
  const setGatewayBalanceStatus = usePortfolioStore(
    (s) => s.setGatewayBalanceStatus,
  );
  const [savedEmail, setSavedEmail] = useState("");
  const [checkingWallet, setCheckingWallet] = useState(false);
  const [refreshingGateway, setRefreshingGateway] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);

  useEffect(() => {
    setSavedEmail(window.localStorage.getItem("aegis_email") ?? "");
  }, []);

  const eurcUsd =
    snapshot?.assets.find((a) => a.symbol === "EURC")?.priceUsd ?? 1.085;
  const totalUsdEquivalent = unifiedUsdc + unifiedEurc * eurcUsd;
  const balanceLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const balanceUnavailable = gatewayBalanceStatus === "error";
  const refreshGatewayBalance = useCallback(async () => {
    setRefreshingGateway(true);
    setStatusError(null);
    setStatusMessage(null);
    setGatewayBalanceStatus("loading");
    try {
      const balance = await gatewayApi.balance();
      setUnifiedUsdc(balance.unifiedUsdc);
      setUnifiedEurc(balance.unifiedEurc);
      setPerChain(balance.perChain ?? {}, balance.perChainEurc ?? {});
      setGatewayBalanceStatus("ready");
      setStatusMessage(
        "Gateway returned fresh Arc + Base token balances for this wallet.",
      );
    } catch (e) {
      const message = formatGatewayBalanceError(e);
      setGatewayBalanceStatus("error", message);
      setStatusError(message);
    } finally {
      setRefreshingGateway(false);
    }
  }, [setGatewayBalanceStatus, setPerChain, setUnifiedEurc, setUnifiedUsdc]);

  if (!wallet) {
    const resumeHref = savedEmail
      ? `/login?email=${encodeURIComponent(savedEmail)}`
      : "/login";
    const checkStatus = async () => {
      setCheckingWallet(true);
      setStatusError(null);
      setStatusMessage(null);
      try {
        const status = await walletApi.status();
        if (status.wallet) {
          setWallet(status.wallet);
          setStatusMessage(
            "Circle returned both wallet addresses. Wallet page is live.",
          );
        } else {
          setStatusMessage(
            "Circle has not returned both Arc + Base addresses yet. Resume setup if the PIN ceremony did not finish.",
          );
        }
      } catch (e) {
        setStatusError(walletStatusError(e));
      } finally {
        setCheckingWallet(false);
      }
    };
    return (
      <div className="mx-auto grid max-w-[1200px] gap-6 xl:grid-cols-[minmax(0,1fr)_420px]">
        <section className="border-brutal border-border-default bg-surface p-5 shadow-brutal">
          <div className="flex flex-wrap items-start justify-between gap-4 border-b border-border-default pb-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
                Wallet gate
              </p>
              <h1 className="mt-1 flex items-center gap-2 font-mono text-2xl font-semibold text-text-hi">
                <WalletIcon className="h-5 w-5 text-accent-agent" />
                {sessionActive
                  ? "Finish Circle wallet setup"
                  : "Sign in before wallet access"}
              </h1>
              <p className="mt-2 max-w-2xl font-mono text-xs leading-relaxed text-text-lo">
                {sessionActive
                  ? "Aegis has a server-verified session, but execution stays locked until Circle returns real Arc + Base wallet addresses."
                  : "Wallet addresses, Gateway balances, faucet claims, deployments, and rebalances require a server-verified session first."}
              </p>
            </div>
            <span
              className={`inline-flex min-h-8 items-center gap-2 rounded-sharp border px-3 font-mono text-[10px] uppercase tracking-widest ${
                sessionActive
                  ? "border-warn/40 bg-warn/5 text-warn"
                  : "border-border-default bg-bg text-text-mut"
              }`}
            >
              {sessionActive ? (
                <CircleAlert className="h-3.5 w-3.5" />
              ) : (
                <LogIn className="h-3.5 w-3.5" />
              )}
              {sessionActive ? "Wallet pending" : "Signed out"}
            </span>
          </div>

          <WalletSetupSvg sessionActive={sessionActive} />

          <div className="mt-5 grid gap-3 md:grid-cols-3">
            <GateFact
              label="Session"
              value={sessionActive ? "verified" : "required"}
              tone={sessionActive ? "agent" : "neutral"}
            />
            <GateFact
              label="Circle wallet"
              value="not ready"
              tone={sessionActive ? "warn" : "neutral"}
            />
            <GateFact label="Execution" value="blocked" tone="warn" />
          </div>

          <div className="mt-5 flex flex-col gap-3 sm:flex-row">
            {sessionActive ? (
              <button
                type="button"
                onClick={() => void checkStatus()}
                disabled={checkingWallet}
                className="inline-flex min-h-10 flex-1 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-agent px-4 font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none"
              >
                {checkingWallet ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <RotateCw className="h-4 w-4" />
                )}
                Check Circle status
              </button>
            ) : (
              <Link
                href={resumeHref}
                className="inline-flex min-h-10 flex-1 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-agent px-4 font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
              >
                <LogIn className="h-4 w-4" />
                Sign in
              </Link>
            )}
            <Link
              href={resumeHref}
              className="inline-flex min-h-10 flex-1 items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-4 font-mono text-sm text-text-lo hover:border-border-hi hover:text-text-hi"
            >
              {sessionActive ? (
                <>
                  Resume PIN setup
                  <ArrowRight className="h-4 w-4" />
                </>
              ) : (
                <>
                  Restore wallet
                  <ArrowRight className="h-4 w-4" />
                </>
              )}
            </Link>
            <Link
              href="/signup"
              className="inline-flex min-h-10 flex-1 items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-4 font-mono text-sm text-text-lo hover:border-border-hi hover:text-text-hi"
            >
              <UserPlus className="h-4 w-4" />
              Create wallet
            </Link>
          </div>

          {(statusMessage || statusError) && (
            <p
              role={statusError ? "alert" : "status"}
              className={`mt-4 border px-3 py-2 font-mono text-[11px] leading-relaxed ${
                statusError
                  ? "border-risk/40 bg-risk/5 text-risk"
                  : "border-accent-agent/40 bg-accent-agent/5 text-text-lo"
              }`}
            >
              {statusError ?? statusMessage}
            </p>
          )}
        </section>

        <aside className="border-brutal border-border-default bg-raised p-5 font-mono">
          <p className="text-[10px] uppercase tracking-widest text-text-mut">
            What unlocks next
          </p>
          <div className="mt-4 space-y-3 text-xs">
            <UnlockStep
              active={sessionActive}
              title="1. Server session"
              body="The backend must accept the HttpOnly Aegis session cookie. Stale browser hints are ignored."
            />
            <UnlockStep
              active={false}
              title="2. Arc + Base addresses"
              body="Circle must return both chain wallets before Gateway balances or CCTP routes can be trusted."
            />
            <UnlockStep
              active={false}
              title="3. Fund wallet cash"
              body="USDC appears here first. Dashboard shows invested positions after a reviewed deployment confirms."
            />
          </div>
        </aside>
      </div>
    );
  }

  const chains: Array<{
    key: ExplorerChain;
    label: string;
    address: string;
  }> = [
    {
      key: "arc",
      label: "Arc Testnet",
      address: wallet.arcAddress,
    },
    {
      key: "base",
      label: "Base Sepolia",
      address: wallet.baseAddress,
    },
  ];

  const isEmpty =
    gatewayBalanceStatus === "ready" &&
    unifiedUsdc < 0.01 &&
    unifiedEurc < 0.01;

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight flex items-center gap-2">
            <WalletIcon className="w-5 h-5 text-accent-pnl" />
            Wallets
          </h1>
          <p className="text-sm text-text-lo mt-1">
            Idle Circle Gateway cash only. Invested positions stay on Dashboard
            and Portfolio.
          </p>
        </div>
        {isEmpty && <FaucetButton />}
      </div>

      <WalletOperationalPanel
        gatewayBalanceStatus={gatewayBalanceStatus}
        refreshingGateway={refreshingGateway}
        onRefreshGateway={() => void refreshGatewayBalance()}
      />

      {balanceUnavailable && (
        <div className="border-brutal border-warn/50 bg-warn/5 p-4 font-mono">
          <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
            <div className="flex items-start gap-3">
              <CircleAlert className="mt-0.5 h-5 w-5 shrink-0 text-warn" />
              <div>
                <p className="text-sm font-semibold text-text-hi">
                  Gateway balance is unknown
                </p>
                <p className="mt-1 text-xs leading-relaxed text-text-lo">
                  {gatewayBalanceError ??
                    "Circle Gateway did not return Arc + Base balances."}{" "}
                  Aegis is showing wallet addresses, but cash values and faucet
                  prompts stay hidden until Gateway confirms them. Do not treat
                  this as a $0 wallet.
                </p>
              </div>
            </div>
            <button
              type="button"
              onClick={() => void refreshGatewayBalance()}
              disabled={refreshingGateway}
              className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 rounded-sharp border border-warn/50 bg-warn/10 px-4 font-mono text-xs font-semibold text-warn hover:bg-warn/15 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {refreshingGateway ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RotateCw className="h-3.5 w-3.5" />
              )}
              Retry Gateway check
            </button>
          </div>
        </div>
      )}

      {(statusMessage || statusError) && (
        <p
          role={statusError ? "alert" : "status"}
          className={`border px-3 py-2 font-mono text-[11px] leading-relaxed ${
            statusError
              ? "border-risk/40 bg-risk/5 text-risk"
              : "border-accent-agent/40 bg-accent-agent/5 text-text-lo"
          }`}
        >
          {statusError ?? statusMessage}
        </p>
      )}

      <BrutalCard>
        <BrutalCardHeader>
          <span className="text-sm font-mono text-text-hi">
            Idle wallet cash
          </span>
        </BrutalCardHeader>
        <BrutalCardBody>
          <p
            className={`font-mono font-semibold tabular-nums ${
              balanceUnavailable
                ? "text-xl text-warn"
                : balanceLoading
                  ? "text-xl text-text-lo"
                  : "text-2xl text-accent-pnl"
            }`}
          >
            {balanceUnavailable
              ? "Balance unavailable"
              : balanceLoading
                ? "Checking Gateway..."
                : formatCurrency(totalUsdEquivalent)}
          </p>
          <p className="text-xs font-mono text-text-lo mt-1">
            {balanceUnavailable
              ? "Gateway check failed before returning token balances"
              : balanceLoading
                ? "Waiting for Circle Gateway to confirm Arc + Base cash"
                : `${formatCurrency(unifiedUsdc)} USDC · €${unifiedEurc.toFixed(2)} EURC`}
            {!balanceUnavailable && !balanceLoading && unifiedEurc > 0 && (
              <span className="text-text-mut">
                {" "}
                (≈ {formatCurrency(unifiedEurc * eurcUsd)})
              </span>
            )}
          </p>
          <p className="text-[11px] font-mono text-text-mut mt-3">
            {balanceUnavailable
              ? "Wallet cash is unknown because Gateway did not confirm chain balances. Use the retry check above after Circle API connectivity is restored."
              : balanceLoading
                ? "Aegis is checking Gateway before enabling cash actions."
                : isEmpty
                  ? "This can be $0 even when you own investments. Deployed positions are counted on Dashboard and Portfolio; newly funded USDC appears here first."
                  : "This is spendable cash that has not been invested yet. Review any deployment or rebalance plan before real execution."}
          </p>
        </BrutalCardBody>
      </BrutalCard>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {chains.map((c) => (
          <ChainCard
            key={c.key}
            chain={c.key}
            label={c.label}
            address={c.address}
            usdc={perChainUsdc[c.key] ?? 0}
            eurc={perChainEurc[c.key] ?? 0}
            eurcUsd={eurcUsd}
            balanceStatus={gatewayBalanceStatus}
          />
        ))}
      </div>
    </div>
  );
}

function WalletOperationalPanel({
  gatewayBalanceStatus,
  refreshingGateway,
  onRefreshGateway,
}: {
  gatewayBalanceStatus: "idle" | "loading" | "ready" | "error";
  refreshingGateway: boolean;
  onRefreshGateway: () => void;
}) {
  const gatewayReady = gatewayBalanceStatus === "ready";
  const gatewayLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const gatewayFailed = gatewayBalanceStatus === "error";
  return (
    <section className="grid gap-4 border-brutal border-border-default bg-surface p-4 font-mono shadow-brutal md:grid-cols-[minmax(0,1fr)_360px]">
      <div>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-widest text-accent-agent">
              Wallet operational status
            </p>
            <h2 className="mt-1 text-base font-semibold text-text-hi">
              Address custody is ready. Cash visibility depends on Gateway.
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
            {gatewayLoading ? "Checking Gateway" : "Refresh Gateway"}
          </button>
        </div>
        <div className="mt-4 grid gap-3 sm:grid-cols-4">
          <GateFact label="Session" value="verified" tone="agent" />
          <GateFact label="Arc wallet" value="ready" tone="pnl" />
          <GateFact label="Base wallet" value="ready" tone="pnl" />
          <GateFact
            label="Gateway cash"
            value={
              gatewayReady ? "verified" : gatewayFailed ? "unknown" : "checking"
            }
            tone={gatewayReady ? "pnl" : gatewayFailed ? "warn" : "neutral"}
          />
        </div>
        <p className="mt-3 text-xs leading-relaxed text-text-lo">
          Copy addresses and receive funds any time. Deploy, faucet prompts, and
          rebalance execution stay locked until Gateway confirms spendable
          USDC/EURC balances, so an outage cannot be misread as a true zero.
        </p>
      </div>
      <WalletOpsSvg
        gatewayReady={gatewayReady}
        gatewayFailed={gatewayFailed}
        gatewayLoading={gatewayLoading}
      />
    </section>
  );
}

function WalletOpsSvg({
  gatewayReady,
  gatewayFailed,
  gatewayLoading,
}: {
  gatewayReady: boolean;
  gatewayFailed: boolean;
  gatewayLoading: boolean;
}) {
  const gatewayStroke = gatewayReady
    ? "#86efac"
    : gatewayFailed
      ? "#f59e0b"
      : "#67e8f9";
  const caption = gatewayReady
    ? ["cash verified", "reviews can price real routes"]
    : gatewayFailed
      ? ["cash unknown", "retry Gateway check"]
      : ["checking cash", "actions unlock after confirmation"];
  return (
    <svg
      viewBox="0 0 360 190"
      role="img"
      aria-label="Wallet readiness map from wallets through Gateway to execution"
      className="h-auto w-full border border-border-default bg-bg"
    >
      <defs>
        <pattern
          id="wallet-ops-grid"
          width="20"
          height="20"
          patternUnits="userSpaceOnUse"
        >
          <path d="M20 0H0V20" fill="none" stroke="#242424" strokeWidth="1" />
        </pattern>
      </defs>
      <rect width="360" height="190" fill="url(#wallet-ops-grid)" />
      <path
        d="M72 92H152M208 92H288"
        fill="none"
        stroke={gatewayStroke}
        strokeDasharray="9 7"
        strokeWidth="4"
      >
        {(gatewayReady || gatewayLoading) && (
          <animate
            attributeName="stroke-dashoffset"
            dur={gatewayLoading ? "1.4s" : "2.4s"}
            from="32"
            repeatCount="indefinite"
            to="0"
          />
        )}
      </path>
      <MiniOpsNode x={28} y={54} label="WALLETS" tone="agent" />
      <MiniOpsNode
        x={140}
        y={54}
        label={gatewayFailed ? "UNKNOWN" : "GATEWAY"}
        tone={gatewayFailed ? "warn" : gatewayReady ? "pnl" : "agent"}
      />
      <MiniOpsNode
        x={252}
        y={54}
        label={gatewayReady ? "UNLOCK" : "LOCKED"}
        tone={gatewayReady ? "pnl" : "warn"}
      />
      <g transform="translate(96 142)">
        <rect
          width="168"
          height="34"
          fill="#111111"
          stroke={gatewayStroke}
          strokeWidth="1.5"
        />
        <rect x="10" y="9" width="14" height="14" fill={gatewayStroke}>
          {(gatewayReady || gatewayLoading) && (
            <animate
              attributeName="opacity"
              dur={gatewayLoading ? "0.8s" : "1.8s"}
              repeatCount="indefinite"
              values="0.35;1;0.35"
            />
          )}
        </rect>
        <text
          x="34"
          y="14"
          fill={gatewayFailed ? "#f59e0b" : "#f5f5f5"}
          fontFamily="monospace"
          fontSize="9"
          fontWeight="700"
        >
          {caption[0]}
        </text>
        <text x="34" y="26" fill="#a3a3a3" fontFamily="monospace" fontSize="8">
          {caption[1]}
        </text>
      </g>
    </svg>
  );
}

function MiniOpsNode({
  x,
  y,
  label,
  tone,
}: {
  x: number;
  y: number;
  label: string;
  tone: "agent" | "pnl" | "warn";
}) {
  const color =
    tone === "agent" ? "#67e8f9" : tone === "pnl" ? "#86efac" : "#f59e0b";
  return (
    <g>
      <rect x={x} y={y} width="80" height="76" fill="#111111" stroke={color} />
      <rect x={x + 12} y={y + 12} width="56" height="16" fill={color} />
      <text
        x={x + 40}
        y={y + 52}
        fill="#f5f5f5"
        fontFamily="monospace"
        fontSize="11"
        fontWeight="700"
        textAnchor="middle"
      >
        {label}
      </text>
    </g>
  );
}

function GateFact({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "agent" | "pnl" | "warn" | "neutral";
}) {
  return (
    <div
      className={`border px-3 py-2 font-mono ${
        tone === "agent"
          ? "border-accent-agent/40 bg-accent-agent/5"
          : tone === "pnl"
            ? "border-accent-pnl/40 bg-accent-pnl/5"
            : tone === "warn"
              ? "border-warn/40 bg-warn/5"
              : "border-border-default bg-bg"
      }`}
    >
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p
        className={`mt-1 text-xs ${
          tone === "agent"
            ? "text-accent-agent"
            : tone === "pnl"
              ? "text-accent-pnl"
              : tone === "warn"
                ? "text-warn"
                : "text-text-hi"
        }`}
      >
        {value}
      </p>
    </div>
  );
}

function UnlockStep({
  active,
  title,
  body,
}: {
  active: boolean;
  title: string;
  body: string;
}) {
  return (
    <div className="grid grid-cols-[24px_1fr] gap-3 border border-border-default bg-bg p-3">
      <span
        className={`mt-0.5 flex h-5 w-5 items-center justify-center rounded-sharp border ${
          active
            ? "border-accent-agent/50 bg-accent-agent/10 text-accent-agent"
            : "border-border-default text-text-mut"
        }`}
      >
        {active ? (
          <Check className="h-3 w-3" />
        ) : (
          <span className="h-1.5 w-1.5 bg-text-mut" />
        )}
      </span>
      <span>
        <span className="block font-semibold text-text-hi">{title}</span>
        <span className="mt-1 block leading-relaxed text-text-lo">{body}</span>
      </span>
    </div>
  );
}

function WalletSetupSvg({ sessionActive }: { sessionActive: boolean }) {
  return (
    <svg
      viewBox="0 0 760 260"
      role="img"
      aria-label="Wallet setup path from session to Arc and Base wallets"
      className="mt-5 h-auto w-full border border-border-default bg-bg"
    >
      <defs>
        <pattern
          id="wallet-setup-grid"
          width="24"
          height="24"
          patternUnits="userSpaceOnUse"
        >
          <path d="M24 0H0V24" fill="none" stroke="#242424" strokeWidth="1" />
        </pattern>
        <filter
          id="wallet-setup-glow"
          x="-25%"
          y="-25%"
          width="150%"
          height="150%"
        >
          <feGaussianBlur stdDeviation="3" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <rect width="760" height="260" fill="url(#wallet-setup-grid)" />
      <path
        d="M130 130H302M458 130H625"
        fill="none"
        stroke={sessionActive ? "#67e8f9" : "#525252"}
        strokeDasharray="10 8"
        strokeWidth="4"
        filter={sessionActive ? "url(#wallet-setup-glow)" : undefined}
      >
        {sessionActive && (
          <animate
            attributeName="stroke-dashoffset"
            dur="2.2s"
            from="36"
            repeatCount="indefinite"
            to="0"
          />
        )}
      </path>
      <SetupNode
        x={62}
        y={84}
        title="Session"
        subtitle={sessionActive ? "server OK" : "missing"}
        tone={sessionActive ? "agent" : "neutral"}
      />
      <SetupNode
        x={302}
        y={84}
        title="Circle PIN"
        subtitle="browser SDK"
        tone={sessionActive ? "warn" : "neutral"}
      />
      <SetupNode x={578} y={48} title="Arc" subtitle="USDC gas" tone="pnl" />
      <SetupNode x={578} y={148} title="Base" subtitle="CCTP" tone="pnl" />
      <path
        d="M458 130C510 130 512 86 578 86M458 130C510 130 512 186 578 186"
        fill="none"
        stroke="#86efac"
        strokeDasharray="6 7"
        strokeWidth="3"
      >
        <animate
          attributeName="stroke-dashoffset"
          dur="2.8s"
          from="26"
          repeatCount="indefinite"
          to="0"
        />
      </path>
      {!sessionActive && (
        <g>
          <circle cx="217" cy="130" r="22" fill="#737373" opacity="0.18" />
          <path
            d="M207 120L227 140M227 120L207 140"
            stroke="#737373"
            strokeLinecap="square"
            strokeWidth="5"
          />
        </g>
      )}
    </svg>
  );
}

function SetupNode({
  x,
  y,
  title,
  subtitle,
  tone,
}: {
  x: number;
  y: number;
  title: string;
  subtitle: string;
  tone: "agent" | "pnl" | "warn" | "neutral";
}) {
  const stroke =
    tone === "agent"
      ? "#67e8f9"
      : tone === "pnl"
        ? "#86efac"
        : tone === "warn"
          ? "#f59e0b"
          : "#525252";
  const fill =
    tone === "agent"
      ? "#082f49"
      : tone === "pnl"
        ? "#052e16"
        : tone === "warn"
          ? "#2a1d06"
          : "#111111";
  return (
    <g>
      <rect
        x={x}
        y={y}
        width="120"
        height="92"
        fill={fill}
        stroke={stroke}
        strokeWidth="3"
      />
      <rect x={x + 14} y={y + 14} width="92" height="14" fill={stroke} />
      <text
        x={x + 60}
        y={y + 56}
        fill="#f5f5f5"
        fontFamily="monospace"
        fontSize="15"
        fontWeight="700"
        textAnchor="middle"
      >
        {title}
      </text>
      <text
        x={x + 60}
        y={y + 74}
        fill="#a3a3a3"
        fontFamily="monospace"
        fontSize="10"
        textAnchor="middle"
      >
        {subtitle}
      </text>
    </g>
  );
}

interface ChainCardProps {
  chain: ExplorerChain;
  label: string;
  address: string;
  usdc: number;
  eurc: number;
  eurcUsd: number;
  balanceStatus: "idle" | "loading" | "ready" | "error";
}

function ChainCard({
  chain,
  label,
  address,
  usdc,
  eurc,
  eurcUsd,
  balanceStatus,
}: ChainCardProps) {
  const addressRef = useRef<HTMLElement>(null);
  const [copyState, setCopyState] = useState<
    "idle" | "copied" | "selected" | "failed"
  >("idle");
  const total = usdc + eurc * eurcUsd;
  const explorerHref = explorerAddressUrl(chain, address);
  const balanceKnown = balanceStatus === "ready";
  const balanceUnavailable = balanceStatus === "error";

  const handleCopy = async () => {
    try {
      if (!navigator.clipboard?.writeText) throw new Error("clipboard missing");
      await navigator.clipboard.writeText(address);
      setCopyState("copied");
      setTimeout(() => setCopyState("idle"), 1800);
    } catch {
      if (copyAddressFallback(address)) {
        setCopyState("copied");
        setTimeout(() => setCopyState("idle"), 1800);
        return;
      }
      if (selectAddress(addressRef.current)) {
        setCopyState("selected");
        setTimeout(() => setCopyState("idle"), 2600);
        return;
      }
      setCopyState("failed");
      setTimeout(() => setCopyState("idle"), 2600);
    }
  };

  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-sm font-mono text-text-hi">{label}</span>
        <span
          className={`text-sm font-mono tabular-nums ${
            balanceUnavailable ? "text-warn" : "text-accent-pnl"
          }`}
        >
          {balanceUnavailable
            ? "unknown"
            : balanceKnown
              ? formatCurrency(total)
              : "checking"}
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
              USDC
            </p>
            <p className="text-sm font-mono text-text-hi tabular-nums">
              {balanceKnown ? formatCurrency(usdc) : "--"}
            </p>
          </div>
          <div className="p-3 rounded-sharp bg-raised border border-border-default">
            <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
              EURC
            </p>
            <p className="text-sm font-mono text-text-hi tabular-nums">
              {balanceKnown ? `€${eurc.toFixed(2)}` : "--"}
            </p>
          </div>
        </div>

        <div>
          <p className="text-[10px] text-text-mut font-mono uppercase tracking-wider mb-1">
            Funding address
          </p>
          <div className="grid gap-2">
            <code
              ref={addressRef}
              tabIndex={0}
              className="block min-w-0 rounded-sharp border border-border-default bg-bg px-2 py-2 text-[11px] font-mono text-text-default break-all"
              title={address}
            >
              {address}
            </code>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => void handleCopy()}
                className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-white/10 bg-white/5 px-3 text-xs font-mono text-text-default hover:border-accent-pnl/40 hover:text-accent-pnl"
                title="Copy address"
                aria-label={`Copy ${label} address`}
              >
                {copyState === "copied" ? (
                  <Check className="w-3.5 h-3.5 text-accent-pnl" />
                ) : copyState === "selected" ? (
                  <Check className="w-3.5 h-3.5 text-warn" />
                ) : (
                  <Copy className="w-3.5 h-3.5" />
                )}
                {copyState === "copied"
                  ? "Copied"
                  : copyState === "selected"
                    ? "Selected"
                    : copyState === "failed"
                      ? "Copy failed"
                      : "Copy"}
              </button>
              {explorerHref && (
                <a
                  href={explorerHref}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-white/10 bg-white/5 px-3 text-xs font-mono text-text-default hover:border-accent-agent/40 hover:text-accent-agent"
                  title="View on explorer"
                  aria-label={`View ${label} on explorer`}
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  Explorer
                </a>
              )}
            </div>
          </div>
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}

function copyAddressFallback(address: string) {
  const textarea = document.createElement("textarea");
  textarea.value = address;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  document.body.appendChild(textarea);
  textarea.select();
  try {
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    document.body.removeChild(textarea);
  }
}

function selectAddress(node: HTMLElement | null) {
  if (!node) return false;
  const range = document.createRange();
  range.selectNodeContents(node);
  const selection = window.getSelection();
  if (!selection) return false;
  selection.removeAllRanges();
  selection.addRange(range);
  node.focus();
  return true;
}

function walletStatusError(error: unknown) {
  const raw = (error as Error).message || "wallet status check failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("missing token") || lower.includes("unauthorized")) {
    return "The server session is not active anymore. Sign in and verify a fresh one-time code before checking wallet setup.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "Aegis could not reach the API. Check that the backend is running, then try again.";
  }
  return message;
}

function formatGatewayBalanceError(error: unknown) {
  const raw = (error as Error).message || "Gateway balance check failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("session expired") || lower.includes("unauthorized")) {
    return "Session expired before Gateway replied. Sign in again before checking balances.";
  }
  if (lower.includes("returned no wallets")) {
    return "Circle Gateway returned no wallets for this provisioned account, so wallet cash is unknown.";
  }
  if (lower.includes("gateway") || lower.includes("circle")) {
    return "Circle Gateway balance check failed.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "Aegis could not reach the API while checking Gateway balances.";
  }
  return message;
}
