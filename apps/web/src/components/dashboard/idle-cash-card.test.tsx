import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { Portfolio, WalletInfo } from "@/types";
import { usePortfolioStore } from "@/stores/portfolio";
import { IdleCashCard } from "./idle-cash-card";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
});

describe("<IdleCashCard />", () => {
  it("keeps the empty-wallet state compact instead of listing zero routes", () => {
    seedEmptyWalletState();

    const { container, root } = render(<IdleCashCard />);
    const text = container.textContent ?? "";

    expect(text).toContain("$0.00");
    expect(text).toContain("No USDC or EURC detected on active wallet routes.");
    expect(text).not.toContain("0.00 USDC");
    expect(text).not.toContain("empty wallet routes hidden");

    act(() => root.unmount());
  });

  it("labels EURC balances as token units in route rows", () => {
    seedEmptyWalletState();
    act(() => {
      const store = usePortfolioStore.getState();
      store.setUnifiedUsdc(20);
      store.setUnifiedEurc(20);
      store.setPerChain({ arc: 20 }, { arc: 20 });
    });

    const { container, root } = render(<IdleCashCard />);
    const text = container.textContent ?? "";

    expect(text).toContain("20.00 EURC");
    expect(text).not.toContain("€20.00");

    act(() => root.unmount());
  });
});

function render(element: React.ReactElement): {
  container: HTMLDivElement;
  root: Root;
} {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() => root.render(element));
  return { container, root };
}

function seedEmptyWalletState() {
  const now = new Date().toISOString();
  const portfolio: Portfolio = {
    id: "portfolio-1",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [],
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
  const store = usePortfolioStore.getState();
  act(() => {
    store.setPortfolios([portfolio]);
    store.setActivePortfolio(portfolio.id);
    store.setWallet(wallet());
    store.setUnifiedUsdc(0);
    store.setUnifiedEurc(0);
    store.setPerChain(
      {
        arc: 0,
        base: 0,
        "arb-sepolia": 0,
      },
      {},
    );
    store.setGatewayBalanceStatus("ready");
  });
}

function wallet(): WalletInfo {
  return {
    walletId: "circle-wallet-1",
    arcAddress: "0x1234567890abcdef1234567890abcdef12345678",
    baseAddress: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
    networks: [
      {
        blockchain: "ARC-TESTNET",
        walletId: "arc-wallet",
        address: "0x1234567890abcdef1234567890abcdef12345678",
        accountType: "EOA",
        state: "LIVE",
      },
      {
        blockchain: "BASE-SEPOLIA",
        walletId: "base-wallet",
        address: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
        accountType: "EOA",
        state: "LIVE",
      },
    ],
    createdAt: new Date().toISOString(),
  };
}
