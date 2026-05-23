import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { Portfolio } from "@/types";
import { usePortfolioStore } from "@/stores/portfolio";
import { dashboardDestination } from "./dashboard-routing";
import DashboardIndex from "./page";

const routerReplace = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    replace: routerReplace,
  }),
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
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
  window.localStorage.clear();
});

describe("dashboardDestination", () => {
  it("prefers the active portfolio when it still exists", () => {
    expect(
      dashboardDestination([portfolio("p1"), portfolio("p2")], "p2")?.id,
    ).toBe("p2");
  });

  it("falls back to the first portfolio when the stored active id is stale", () => {
    expect(dashboardDestination([portfolio("p1")], "missing")?.id).toBe("p1");
  });
});

describe("<DashboardIndex />", () => {
  it("redirects the bare dashboard route to the single portfolio", async () => {
    usePortfolioStore
      .getState()
      .setPortfolios([portfolio("p1", "Primary"), portfolio("p2", "Treasury")]);

    const { container, root } = render(<DashboardIndex />);
    await flushEffects();

    expect(routerReplace).toHaveBeenCalledWith("/dashboard/p1");
    expect(container.textContent).toContain("Opening dashboard");
    expect(container.textContent).toContain("Taking you to Primary.");
    expect(
      container.querySelector<HTMLAnchorElement>('a[href="/dashboard/p1"]')
        ?.textContent,
    ).toContain("Open dashboard");

    act(() => root.unmount());
  });

  it("sends an account with no portfolio to onboarding with setup copy", async () => {
    usePortfolioStore.getState().setPortfolios([]);

    const { container, root } = render(<DashboardIndex />);
    await flushEffects();

    expect(routerReplace).toHaveBeenCalledWith("/onboarding");
    expect(container.textContent).toContain("Finish portfolio setup");
    expect(container.textContent).toContain("Continue setup");
    expect(
      container.querySelector<HTMLAnchorElement>('a[href="/onboarding"]')
        ?.textContent,
    ).toContain("Continue setup");

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

function portfolio(id: string, name = "Main portfolio"): Portfolio {
  const now = new Date().toISOString();
  return {
    id,
    userId: "user-1",
    name,
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [],
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
}
