import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import AgentStudioPage from "./page";
import { usePortfolioStore } from "@/stores/portfolio";
import type { Portfolio } from "@/types";

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
  agentApi: {
    analyze: vi.fn(),
  },
  userAgentApi: {
    status: vi.fn().mockResolvedValue({ pausedAt: null }),
    pause: vi.fn(),
    resume: vi.fn(),
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
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
});

describe("<AgentStudioPage />", () => {
  it("does not call the strategist when there is no cash or invested value", async () => {
    act(() => {
      const store = usePortfolioStore.getState();
      store.setWallet({
        walletId: "wallet-1",
        arcAddress: "0x1111111111111111111111111111111111111111",
        baseAddress: "0x2222222222222222222222222222222222222222",
        createdAt: new Date().toISOString(),
      });
      store.setGatewayBalanceStatus("ready");
      store.setUnifiedUsdc(0);
      store.setUnifiedEurc(0);
      store.setPortfolios([emptyPortfolio()]);
    });

    const { root, container } = render(<AgentStudioPage />);
    await flushEffects();

    expect(container.textContent).toContain("Recommendation locked");
    expect(container.textContent).toContain(
      "Add wallet cash or hold an invested position before asking for a recommendation.",
    );
    expect(container.querySelector<HTMLButtonElement>("button")?.disabled).toBe(
      false,
    );
    const recommendationButton = Array.from(
      container.querySelectorAll<HTMLButtonElement>("button"),
    ).find((button) => button.textContent?.includes("Recommendation locked"));
    expect(recommendationButton?.disabled).toBe(true);

    act(() => root.unmount());
  });
});

function emptyPortfolio(): Portfolio {
  return {
    id: "portfolio-1",
    userId: "user-1",
    name: "Empty portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    riskScore: 0,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    allocations: [
      {
        assetId: "btc",
        symbol: "BTC",
        quantity: 0,
        targetWeight: 50,
        currentWeight: 0,
        valueUsd: 0,
      },
    ],
    goal: {
      name: "Growth",
      horizon: "5y",
      riskTolerance: "moderate",
      targetAllocation: { BTC: 50 },
      includeUsyc: false,
      includeEurc: false,
      createdAt: new Date().toISOString(),
    },
  };
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

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
