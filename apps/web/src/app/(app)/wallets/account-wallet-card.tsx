"use client";

import { useRef, useState } from "react";
import { Check, Copy, ExternalLink } from "lucide-react";
import { BrutalCard, BrutalCardBody, BrutalCardHeader } from "@aegis/ui";
import { copyTextToClipboard } from "@/lib/clipboard";
import { explorerAddressUrl, type ExplorerChain } from "@/lib/explorers";

interface AccountWalletCardProps {
  accountAddress: string | null;
  networks: string[];
  explorerLinks: Array<{
    key: ExplorerChain;
    label: string;
    address: string;
  }>;
}

export function AccountWalletCard({
  accountAddress,
  networks,
  explorerLinks,
}: AccountWalletCardProps) {
  const addressRef = useRef<HTMLElement>(null);
  const [copyState, setCopyState] = useState<
    "idle" | "copied" | "selected" | "failed"
  >("idle");
  const sharedExplorerLinks = accountAddress
    ? explorerLinks.filter(
        (link) => link.address.toLowerCase() === accountAddress.toLowerCase(),
      )
    : [];

  const handleCopy = async () => {
    if (!accountAddress) return;
    try {
      await copyTextToClipboard(accountAddress);
      setCopyState("copied");
      setTimeout(() => setCopyState("idle"), 1800);
    } catch {
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
        <span className="text-sm font-mono text-text-hi">Wallet address</span>
        <span className="text-xs font-mono text-accent-agent">
          {networks.length} networks
        </span>
      </BrutalCardHeader>
      <BrutalCardBody className="space-y-3">
        <p className="text-xs leading-relaxed text-text-lo">
          {accountAddress
            ? "Use this one address on any supported network below."
            : "This account uses separate addresses on some networks."}
        </p>
        {accountAddress ? (
          <div className="grid gap-2">
            <code
              ref={addressRef}
              tabIndex={0}
              className="block min-w-0 rounded-sharp border border-border-default bg-bg px-3 py-2 text-[11px] font-mono text-text-default break-all"
              title={accountAddress}
            >
              {accountAddress}
            </code>
            <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
              <button
                type="button"
                onClick={() => void handleCopy()}
                className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-white/10 bg-white/5 px-3 text-xs font-mono text-text-default hover:border-accent-pnl/40 hover:text-accent-pnl"
                title="Copy wallet address"
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
                    ? "Address selected"
                    : copyState === "failed"
                      ? "Copy failed"
                      : "Copy address"}
              </button>
              {sharedExplorerLinks.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {sharedExplorerLinks.map((link) => {
                    const href = explorerAddressUrl(link.key, link.address);
                    if (!href) return null;
                    return (
                      <a
                        key={link.key}
                        href={href}
                        target="_blank"
                        rel="noreferrer"
                        className="inline-flex min-h-9 items-center justify-center gap-2 rounded-sharp border border-white/10 bg-white/5 px-3 text-xs font-mono text-text-default hover:border-accent-agent/40 hover:text-accent-agent"
                      >
                        <ExternalLink className="w-3.5 h-3.5" />
                        {link.label}
                      </a>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        ) : (
          <p className="rounded-sharp border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] text-warn">
            Copy the address from the exact network card before funding.
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
