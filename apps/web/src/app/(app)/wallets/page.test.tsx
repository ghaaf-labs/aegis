import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

import { AccountWalletCard } from "./account-wallet-card";
import { NetworkTokenPanel } from "./network-token-panel";
import { WalletOperationalPanel } from "./wallet-operational-panel";

const clipboardMock = vi.hoisted(() => ({
  copyTextToClipboard: vi.fn(),
}));

vi.mock("@/lib/clipboard", () => clipboardMock);

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  window.localStorage.clear();
  vi.clearAllMocks();
});

describe("<WalletOperationalPanel />", () => {
  it("does not say cash is checking after a loaded zero balance", () => {
    const { container, root } = render(
      <WalletOperationalPanel
        gatewayBalanceStatus="ready"
        idleCashUsd={0}
        refreshingGateway={false}
        onRefreshGateway={vi.fn()}
      />,
    );

    expect(container.textContent).toContain(
      "Wallet ready. No idle cash available.",
    );
    expect(container.textContent).toContain(
      "New USDC appears here before it is invested.",
    );
    expect(container.textContent).not.toContain("checking available cash");

    act(() => root.unmount());
  });

  it("keeps the refresh action enabled after the balance check finishes", () => {
    const onRefreshGateway = vi.fn();
    const { container, root } = render(
      <WalletOperationalPanel
        gatewayBalanceStatus="ready"
        idleCashUsd={12}
        refreshingGateway={false}
        onRefreshGateway={onRefreshGateway}
      />,
    );
    const button = container.querySelector<HTMLButtonElement>("button");

    expect(container.textContent).toContain("Wallet ready. Cash is available.");
    expect(button?.disabled).toBe(false);

    act(() => root.unmount());
  });
});

describe("<AccountWalletCard />", () => {
  const address = "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7";

  it("shows a shared account address once for multi-network wallets", () => {
    const { container, root } = render(
      <AccountWalletCard
        accountAddress={address}
        networks={["Arc testnet", "Base Sepolia"]}
        explorerLinks={[
          { key: "arc", label: "Arc testnet", address },
          { key: "base", label: "Base Sepolia", address },
        ]}
      />,
    );
    const text = container.textContent ?? "";

    expect(text).toContain("Use this one address");
    expect(text).toContain("Arc testnet");
    expect(text).toContain("Base Sepolia");
    expect(text.match(new RegExp(address, "g"))).toHaveLength(1);

    act(() => root.unmount());
  });

  it("uses plain fallback copy when clipboard write is blocked", async () => {
    clipboardMock.copyTextToClipboard.mockRejectedValueOnce(
      new Error("clipboard blocked"),
    );
    const { container, root } = render(
      <AccountWalletCard
        accountAddress={address}
        networks={["Arc testnet", "Base Sepolia"]}
        explorerLinks={[]}
      />,
    );
    const button = container.querySelector<HTMLButtonElement>(
      'button[title="Copy wallet address"]',
    );

    await act(async () => {
      button?.click();
      await Promise.resolve();
    });

    expect(container.textContent).toContain("Address selected");
    expect(container.textContent).toContain(address);
    expect(clipboardMock.copyTextToClipboard).toHaveBeenCalledWith(address);

    act(() => root.unmount());
  });
});

describe("<NetworkTokenPanel />", () => {
  it("lets the user choose wallet routes and target tokens", async () => {
    const onPreferencesChange = vi.fn();
    const { container, root } = render(
      <NetworkTokenPanel
        networks={[
          {
            blockchain: "ARC-TESTNET",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "BASE-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
        ]}
        persistenceLabel="Saved to active portfolio"
        onPreferencesChange={onPreferencesChange}
      />,
    );
    const text = container.textContent ?? "";
    const expectedNetworks = [
      "Arc testnet",
      "Base Sepolia",
      "Ethereum Sepolia",
      "Arbitrum Sepolia",
      "Avalanche Fuji",
    ];
    const expectedTokens = [
      ["USDC", "Cash · Reserve, funding, and transfer route is ready"],
      [
        "BTC",
        "Market target · Price tracking is ready; swap execution is not connected yet",
      ],
      [
        "ETH",
        "Market target · Price tracking is ready; swap execution is not connected yet",
      ],
      [
        "SOL",
        "Market target · Price tracking is ready; swap execution is not connected yet",
      ],
      [
        "USYC",
        "Yield target · Not available in this build; USYC stays visible as a coming-soon route",
      ],
      [
        "EURC",
        "FX target · FX tracking is ready; EURC executes on the Base USDC/EURC pool when the swap rail is live",
      ],
    ];

    for (const network of expectedNetworks) {
      expect(text).toContain(network);
    }
    expect(text).toContain("Current selection");
    expect(text).toContain("Arc testnet, Base Sepolia");
    expect(text).toContain("Can rebalance now");
    expect(text).toContain("BTC, ETH, SOL, EURC");
    expect(text).toContain("Coming soon");
    expect(text).toContain("USYC");
    expect(text).toContain(
      "Ethereum Sepolia, Arbitrum Sepolia, Avalanche Fuji",
    );
    expect(text).toContain("Saved to active portfolio");
    expect(text).toContain("No wallet address yet");
    expect(text).toContain("Selected for balance tracking and rebalances");
    expect(text).toContain(
      "Aegis can prepare real reviews on Arc testnet and Base Sepolia",
    );
    expect(text).toContain("Not ready");
    expect(text).toContain("Use ready routes");
    expect(text).toContain("USDC reserve as the active execution target");
    expect(text).not.toContain("Circle transfer rail");

    for (const tokenCopy of expectedTokens.flat()) {
      expect(text).toContain(tokenCopy);
    }

    const disabledButtons = Array.from(
      container.querySelectorAll<HTMLButtonElement>("button:disabled"),
    );
    expect(disabledButtons).toHaveLength(1);
    expect(disabledButtons[0]?.textContent).toContain("Coming soon");

    await act(async () => {
      findButton(container, "Agent suggestion").click();
      await Promise.resolve();
    });

    const suggestedText = container.textContent ?? "";
    expect(suggestedText).toContain("USDC");
    expect(suggestedText).toContain("BTC, ETH, SOL, EURC");
    expect(suggestedText).toContain(
      "Ethereum Sepolia, Arbitrum Sepolia, Avalanche Fuji",
    );
    expect(onPreferencesChange).toHaveBeenCalledWith({
      networks: ["ARC-TESTNET", "BASE-SEPOLIA"],
      networkWatchlist: ["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"],
      tokens: ["USDC"],
      watchlist: ["BTC", "ETH", "SOL", "EURC"],
    });
    expect(
      window.localStorage.getItem("aegis.wallet.route-preferences.v2"),
    ).toContain("BTC");

    act(() => root.unmount());
  });

  it("marks every provisioned Circle wallet chain as ready", () => {
    const { container, root } = render(
      <NetworkTokenPanel
        networks={[
          {
            blockchain: "ARC-TESTNET",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "BASE-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "ETH-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "ARB-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "AVAX-FUJI",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
        ]}
      />,
    );
    const text = container.textContent ?? "";

    expect(text).toContain(
      "Arc testnet, Base Sepolia, Ethereum Sepolia, Arbitrum Sepolia, Avalanche Fuji",
    );
    expect(text).toContain("Can rebalance now");
    expect(text).toContain("All supported routes ready");
    expect(text).toContain("Selected for balance tracking only");
    expect(text).not.toContain("Not ready");
    expect(text).not.toContain("No wallet address yet");

    act(() => root.unmount());
  });

  it("promotes previously watched networks once their wallet routes exist", () => {
    const { container, root } = render(
      <NetworkTokenPanel
        networks={[
          {
            blockchain: "ARC-TESTNET",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "BASE-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "ETH-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "ARB-SEPOLIA",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
          {
            blockchain: "AVAX-FUJI",
            address: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
          },
        ]}
        initialPreferences={{
          networks: ["ARC-TESTNET", "BASE-SEPOLIA"],
          networkWatchlist: ["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"],
          tokens: ["USDC"],
          watchlist: ["BTC_ETH_SOL", "USYC", "EURC"],
        }}
      />,
    );
    const text = container.textContent ?? "";

    expect(text).toContain(
      "Arc testnet, Base Sepolia, Ethereum Sepolia, Arbitrum Sepolia, Avalanche Fuji",
    );
    expect(text).toContain("USDC");
    expect(text).toContain("BTC, ETH, SOL, EURC");
    expect(text).toContain("Coming soon");
    expect(text).toContain("USYC");
    expect(text).toContain("All supported routes ready");

    act(() => root.unmount());
  });
});

function findButton(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(
    container.querySelectorAll<HTMLButtonElement>("button"),
  ).find((candidate) => candidate.textContent?.includes(text));
  if (!button) {
    throw new Error(`Button not found: ${text}`);
  }
  return button;
}

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
