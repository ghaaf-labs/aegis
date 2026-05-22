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
 * Dedicated wallet view — one account wallet, with per-network token balances
 * and explorer affordances.
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
      setStatusMessage("Fresh wallet cash balance loaded.");
    } catch (e) {
      const message = formatGatewayBalanceError(e);
      setGatewayBalanceStatus("error", message);
      setStatusError(message);
    } finally {
      setRefreshingGateway(false);
    }
  }, [setGatewayBalanceStatus, setPerChain, setUnifiedEurc, setUnifiedUsdc]);

  if (!wallet) {
    const continueHref = sessionActive
      ? "/onboarding"
      : savedEmail
        ? `/login?email=${encodeURIComponent(savedEmail)}`
        : "/login";
    const checkStatus = async () => {
      setCheckingWallet(true);
      setStatusError(null);
      setStatusMessage(null);
      try {
        const session = await walletApi.session();
        if (session.wallet) {
          setWallet(session.wallet);
          setStatusMessage("Account setup is complete. Your wallet is live.");
        } else {
          setStatusMessage(
            "Aegis is still setting up this account. Try again in a moment.",
          );
        }
      } catch (e) {
        setStatusError(walletStatusError(e));
      } finally {
        setCheckingWallet(false);
      }
    };
    return (
      <div className="mx-auto max-w-2xl">
        <section className="border-brutal border-border-default bg-surface p-5 shadow-brutal">
          <div className="flex flex-wrap items-start justify-between gap-4 border-b border-border-default pb-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
                Wallet status
              </p>
              <h1 className="mt-1 flex items-center gap-2 font-mono text-2xl font-semibold text-text-hi">
                <WalletIcon className="h-5 w-5 text-accent-agent" />
                {sessionActive
                  ? "Account setup in progress"
                  : "Continue before wallet access"}
              </h1>
              <p className="mt-2 max-w-2xl font-mono text-xs leading-relaxed text-text-lo">
                {sessionActive
                  ? "Your session is active. Balances, funding, and execution unlock as soon as setup finishes."
                  : "Use one email code before viewing balances or funding addresses."}
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
              {sessionActive ? "Setup pending" : "Signed out"}
            </span>
          </div>

          <div className="mt-5 grid gap-3 md:grid-cols-3">
            <GateFact
              label="Session"
              value={sessionActive ? "verified" : "required"}
              tone={sessionActive ? "agent" : "neutral"}
            />
            <GateFact
              label="Wallet"
              value="setting up"
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
                Check account
              </button>
            ) : (
              <Link
                href={continueHref}
                className="inline-flex min-h-10 flex-1 items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-agent px-4 font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
              >
                <LogIn className="h-4 w-4" />
                Continue
              </Link>
            )}
            <Link
              href={continueHref}
              className="inline-flex min-h-10 flex-1 items-center justify-center gap-2 rounded-sharp border border-border-default bg-bg px-4 font-mono text-sm text-text-lo hover:border-border-hi hover:text-text-hi"
            >
              {sessionActive ? (
                <>
                  Open setup
                  <ArrowRight className="h-4 w-4" />
                </>
              ) : (
                <>Use one email code</>
              )}
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
      </div>
    );
  }

  const allNetworks =
    wallet.networks && wallet.networks.length > 0
      ? wallet.networks
      : [
          {
            blockchain: "ARC-TESTNET",
            walletId: wallet.walletId,
            address: wallet.arcAddress,
            accountType: "SCA",
            state: "LIVE",
          },
          {
            blockchain: "BASE-SEPOLIA",
            walletId: wallet.walletId,
            address: wallet.baseAddress,
            accountType: "SCA",
            state: "LIVE",
          },
        ];
  const chains: Array<{
    key: ExplorerChain;
    label: string;
    address: string;
  }> = allNetworks.flatMap((network) => {
    const route = supportedNetworkRoute(network.blockchain);
    return route ? [{ ...route, address: network.address }] : [];
  });

  const isEmpty =
    gatewayBalanceStatus === "ready" &&
    unifiedUsdc < 0.01 &&
    unifiedEurc < 0.01;
  const uniqueAddresses = Array.from(
    new Set(allNetworks.map((network) => network.address.toLowerCase())),
  );
  const accountAddress =
    uniqueAddresses.length === 1 ? (allNetworks[0]?.address ?? null) : null;

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight flex items-center gap-2">
            <WalletIcon className="w-5 h-5 text-accent-pnl" />
            Wallet
          </h1>
          <p className="text-sm text-text-lo mt-1">
            Your address, available cash, and funding tools in one place. Cash
            shown here is not invested until you approve a plan.
          </p>
        </div>
        {isEmpty && <FaucetButton />}
      </div>

      <WalletOperationalPanel
        gatewayBalanceStatus={gatewayBalanceStatus}
        refreshingGateway={refreshingGateway}
        networkCount={allNetworks.length}
        onRefreshGateway={() => void refreshGatewayBalance()}
      />

      {balanceUnavailable && (
        <div className="border-brutal border-warn/50 bg-warn/5 p-4 font-mono">
          <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
            <div className="flex items-start gap-3">
              <CircleAlert className="mt-0.5 h-5 w-5 shrink-0 text-warn" />
              <div>
                <p className="text-sm font-semibold text-text-hi">
                  Wallet cash is unknown
                </p>
                <p className="mt-1 text-xs leading-relaxed text-text-lo">
                  {gatewayBalanceError ??
                    "The balance check did not return current balances."}{" "}
                  Aegis is showing the wallet address, but cash values and
                  funding prompts stay hidden until the check succeeds. Do not
                  treat this as a $0 wallet.
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
              Retry balance check
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
                ? "Checking balance..."
                : formatCurrency(totalUsdEquivalent)}
          </p>
          <p className="text-xs font-mono text-text-lo mt-1">
            {balanceUnavailable
              ? "Balance check failed before returning token balances"
              : balanceLoading
                ? "Waiting for a current wallet cash check"
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
              ? "Wallet cash is unknown because the balance check did not finish. Use retry after API connectivity is restored."
              : balanceLoading
                ? "Aegis is checking balances before enabling cash actions."
                : isEmpty
                  ? "This can be $0 even when you own investments. Deployed positions are counted on Dashboard and Portfolio; newly funded USDC appears here first."
                  : "This is spendable cash that has not been invested yet. Review any deployment or rebalance plan before real execution."}
          </p>
        </BrutalCardBody>
      </BrutalCard>

      <AccountWalletCard
        accountAddress={accountAddress}
        networks={allNetworks.map((network) =>
          networkLabel(network.blockchain),
        )}
      />

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
  networkCount,
  onRefreshGateway,
}: {
  gatewayBalanceStatus: "idle" | "loading" | "ready" | "error";
  refreshingGateway: boolean;
  networkCount: number;
  onRefreshGateway: () => void;
}) {
  const gatewayReady = gatewayBalanceStatus === "ready";
  const gatewayLoading =
    gatewayBalanceStatus === "idle" || gatewayBalanceStatus === "loading";
  const gatewayFailed = gatewayBalanceStatus === "error";
  return (
    <section className="border-brutal border-border-default bg-surface p-4 font-mono shadow-brutal">
      <div>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-widest text-accent-agent">
              Wallet status
            </p>
            <h2 className="mt-1 text-base font-semibold text-text-hi">
              Wallet is ready. Balance checks confirm available cash.
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
        <div className="mt-4 grid gap-3 sm:grid-cols-4">
          <GateFact label="Session" value="verified" tone="agent" />
          <GateFact label="Wallet" value="ready" tone="pnl" />
          <GateFact label="Routes" value={String(networkCount)} tone="pnl" />
          <GateFact
            label="Cash check"
            value={
              gatewayReady ? "verified" : gatewayFailed ? "unknown" : "checking"
            }
            tone={gatewayReady ? "pnl" : gatewayFailed ? "warn" : "neutral"}
          />
        </div>
        <p className="mt-3 text-xs leading-relaxed text-text-lo">
          Copy the account address any time. Cash actions wait for a fresh
          balance check, so an outage is never shown as a true zero.
        </p>
      </div>
    </section>
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

function supportedNetworkRoute(blockchain: string): {
  key: ExplorerChain;
  label: string;
} | null {
  switch (blockchain) {
    case "ARC-TESTNET":
    case "ARC":
      return { key: "arc", label: "Primary route" };
    case "BASE-SEPOLIA":
    case "BASE":
      return { key: "base", label: "Secondary route" };
    default:
      return null;
  }
}

function networkLabel(blockchain: string) {
  switch (blockchain) {
    case "ARC-TESTNET":
      return "Primary route";
    case "BASE-SEPOLIA":
      return "Secondary route";
    case "ETH-SEPOLIA":
      return "Ethereum route";
    case "MATIC-AMOY":
      return "Polygon route";
    default:
      return blockchain.replaceAll("-", " ");
  }
}

function AccountWalletCard({
  accountAddress,
  networks,
}: {
  accountAddress: string | null;
  networks: string[];
}) {
  return (
    <BrutalCard>
      <BrutalCardHeader>
        <span className="text-sm font-mono text-text-hi">Wallet address</span>
        <span className="text-xs font-mono text-accent-agent">
          {networks.length} routes
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-3">
        <p className="text-xs leading-relaxed text-text-lo">
          Aegis creates one wallet for the user, then enables more routes behind
          the scenes as new networks and tokens are added.
        </p>
        {accountAddress ? (
          <code
            className="block min-w-0 rounded-sharp border border-border-default bg-bg px-3 py-2 text-[11px] font-mono text-text-default break-all"
            title={accountAddress}
          >
            {accountAddress}
          </code>
        ) : (
          <p className="rounded-sharp border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] text-warn">
            This account has route-specific addresses. Aegis still treats them
            as one wallet.
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          {networks.map((network) => (
            <span
              key={network}
              className="rounded-sharp border border-border-default bg-raised px-2 py-1 font-mono text-[10px] uppercase tracking-wider text-text-lo"
            >
              {network}
            </span>
          ))}
        </div>
      </BrutalCardBody>
    </BrutalCard>
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
    return "The server session is not active anymore. Sign in and verify a fresh one-time code before checking account setup.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "Aegis could not reach the API. Check that the backend is running, then try again.";
  }
  return message;
}

function formatGatewayBalanceError(error: unknown) {
  const raw = (error as Error).message || "Balance check failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("session expired") || lower.includes("unauthorized")) {
    return "Session expired before the balance check finished. Sign in again before checking balances.";
  }
  if (lower.includes("returned no wallets")) {
    return "The balance service returned no wallet for this account, so wallet cash is unknown.";
  }
  if (lower.includes("gateway") || lower.includes("circle")) {
    return "Wallet balance check failed.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "Aegis could not reach the API while checking balances.";
  }
  return message;
}
