import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Header } from "./header";
import { usePortfolioStore } from "@/stores/portfolio";
import type { WalletInfo } from "@/types";

let mockPathname = "/dashboard/portfolio-1";

vi.mock("next/navigation", () => ({
  usePathname: () => mockPathname,
}));

vi.mock("next/link", () => ({
  default: ({
    href,
    children,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement> & {
    href: string;
    children: React.ReactNode;
  }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("@/lib/api", () => ({
  gatewayApi: {
    balance: vi.fn().mockResolvedValue({
      unifiedUsdc: 0,
      unifiedEurc: 0,
      perChain: {},
      perChainEurc: {},
    }),
  },
}));

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
  mockPathname = "/dashboard/portfolio-1";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
});

describe("<Header />", () => {
  it("keeps the wallet menu free of duplicate session actions", async () => {
    act(() => {
      const store = usePortfolioStore.getState();
      store.setSessionActive(true);
      store.setWallet(wallet());
    });

    const { container, root } = render(<Header />);
    await flushEffects();

    const walletMenuButton = container.querySelector<HTMLButtonElement>(
      'button[aria-label="Wallet menu"]',
    );
    expect(walletMenuButton).toBeTruthy();

    act(() => {
      walletMenuButton?.dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true }),
      );
    });

    expect(
      container.querySelector('[data-testid="header-logout-direct"]'),
    ).toBeNull();
    expect(container.querySelector('[data-testid="header-logout"]')).toBeNull();
    expect(
      container.querySelectorAll('button[aria-label="Sign out"]'),
    ).toHaveLength(0);
    expect(container.textContent).toContain("Open wallet");

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

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function wallet(): WalletInfo {
  return {
    walletId: "circle-wallet-1",
    arcAddress: "0x1234567890abcdef1234567890abcdef12345678",
    baseAddress: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
    networks: [],
    createdAt: new Date().toISOString(),
  };
}
