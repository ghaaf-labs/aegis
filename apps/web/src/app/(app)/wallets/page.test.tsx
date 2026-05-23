import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

import { AccountWalletCard } from "./account-wallet-card";
import { WalletOperationalPanel } from "./wallet-operational-panel";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
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
  it("shows a shared account address once for multi-network wallets", () => {
    const address = "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7";
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
