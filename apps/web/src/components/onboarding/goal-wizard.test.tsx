import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { GoalWizard } from "./goal-wizard";
import { usePortfolioStore } from "@/stores/portfolio";
import { analyticsApi, portfolioApi } from "@/lib/api";

const push = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push,
  }),
}));

vi.mock("@/lib/api", () => ({
  portfolioApi: {
    create: vi.fn(),
  },
  analyticsApi: {
    track: vi.fn(),
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
  window.sessionStorage.clear();
});

describe("<GoalWizard />", () => {
  it("announces and focuses the portfolio name when Next is pressed empty", async () => {
    const { container, root } = render(<GoalWizard />);

    const next = buttonByText(container, "Next");
    await act(async () => {
      next.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const name = container.querySelector("#portfolio-name");
    expect(document.activeElement).toBe(name);
    expect(name?.getAttribute("aria-invalid")).toBe("true");
    expect(container.querySelectorAll('[role="alert"]')).toHaveLength(1);
    expect(container.textContent).toContain(
      "Enter a portfolio name to continue.",
    );
    expect(container.textContent).not.toContain("tax export");

    act(() => root.unmount());
  });

  it("creates a portfolio with executable route preferences by default", async () => {
    window.sessionStorage.setItem(
      "aegis.goal-wizard.draft",
      JSON.stringify({
        step: 4,
        name: "Main portfolio",
        horizon: "5y",
        risk: "moderate",
        allocation: { USDC: 100, BTC: 0, ETH: 0, SOL: 0, EURC: 0 },
        monthlyContribution: "",
      }),
    );
    vi.mocked(portfolioApi.create).mockResolvedValue({
      id: "portfolio-1",
      userId: "user-1",
      name: "Main portfolio",
      totalValueUsd: 0,
      totalPnlUsd: 0,
      totalPnlPct: 0,
      allocations: [],
      riskScore: 0,
      goal: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    });
    vi.mocked(analyticsApi.track).mockResolvedValue(undefined);
    const { container, root } = render(<GoalWizard />);

    await act(async () => {
      buttonByText(container, "Create portfolio").click();
      await Promise.resolve();
      await Promise.resolve();
    });

    const request = vi.mocked(portfolioApi.create).mock.calls[0]?.[0];
    expect(request?.goal?.targetAllocation).toEqual({
      USDC: 100,
      BTC: 0,
      ETH: 0,
      SOL: 0,
      EURC: 0,
    });
    expect(request?.goal?.routePreferences).toEqual({
      networks: ["ARC-TESTNET", "BASE-SEPOLIA"],
      networkWatchlist: ["ETH-SEPOLIA", "ARB-SEPOLIA", "AVAX-FUJI"],
      tokens: ["USDC"],
      watchlist: ["BTC", "ETH", "SOL", "EURC"],
    });

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

function buttonByText(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find(
    (candidate) => candidate.textContent?.includes(text),
  );
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error(`Missing button: ${text}`);
  }
  return button;
}
