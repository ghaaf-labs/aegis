import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { GoalWizard } from "./goal-wizard";
import { usePortfolioStore } from "@/stores/portfolio";
import { agentApi, analyticsApi, portfolioApi } from "@/lib/api";
import type { Portfolio } from "@/types";

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
  // The wizard no longer calls the allocator; kept here so we can assert it is
  // NOT invoked from onboarding (the dashboard owns the slow design call).
  agentApi: {
    proposeAllocation: vi.fn(),
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

function createdPortfolio(): Portfolio {
  return {
    id: "portfolio-1",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [],
    riskScore: 0,
    goal: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
}

describe("<GoalWizard />", () => {
  it("walks objective → horizon → risk and hands off to the dashboard to design", async () => {
    vi.mocked(portfolioApi.create).mockResolvedValue(createdPortfolio());
    vi.mocked(analyticsApi.track).mockResolvedValue(undefined);

    const { container, root } = render(<GoalWizard />);

    // Step 1 — objective. Default "grow" is preselected; choose "preserve".
    expect(
      container.querySelector('[data-testid="goal-wizard-step-1"]'),
    ).not.toBeNull();
    await clickByText(container, "Preserve");
    await advance(container);

    // Step 2 — horizon.
    expect(
      container.querySelector('[data-testid="goal-wizard-step-2"]'),
    ).not.toBeNull();
    await clickByText(container, "10 years");
    await advance(container);

    // Step 3 — risk. Submit.
    expect(
      container.querySelector('[data-testid="goal-wizard-step-3"]'),
    ).not.toBeNull();
    await clickByText(container, "Conservative");

    await act(async () => {
      buttonByText(container, "Let the agent design it").click();
      await flushMicrotasks();
    });

    const createCall = vi.mocked(portfolioApi.create).mock.calls[0]?.[0];
    expect(createCall?.allocations).toEqual([]);
    expect(createCall?.goal?.targetAllocation).toEqual({});
    expect(createCall?.goal?.name).toBeUndefined();
    expect(createCall?.goal?.objective).toBe("preserve");
    expect(createCall?.goal?.horizon).toBe("10y");
    expect(createCall?.goal?.riskTolerance).toBe("conservative");

    // Onboarding must NOT block on the slow allocator — it navigates straight to
    // the dashboard's designing state, which owns proposal generation.
    expect(agentApi.proposeAllocation).not.toHaveBeenCalled();
    expect(push).toHaveBeenCalledWith("/dashboard/portfolio-1?designing=1");

    act(() => root.unmount());
  });

  it("tracks the goal.completed analytics event before navigating", async () => {
    vi.mocked(portfolioApi.create).mockResolvedValue(createdPortfolio());
    vi.mocked(analyticsApi.track).mockResolvedValue(undefined);

    const { container, root } = render(<GoalWizard />);
    await advance(container); // objective (default "grow")
    await advance(container); // horizon (default "5y")

    await act(async () => {
      buttonByText(container, "Let the agent design it").click();
      await flushMicrotasks();
    });

    expect(analyticsApi.track).toHaveBeenCalledWith(
      "goal.completed",
      expect.objectContaining({ portfolioId: "portfolio-1" }),
    );
    expect(push).toHaveBeenCalledWith("/dashboard/portfolio-1?designing=1");

    act(() => root.unmount());
  });
});

async function advance(container: HTMLElement) {
  await act(async () => {
    buttonByText(container, "Next").click();
    await flushMicrotasks();
  });
}

async function flushMicrotasks() {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
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

function buttonByText(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find(
    (candidate) => candidate.textContent?.includes(text),
  );
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error(`Missing button: ${text}`);
  }
  return button;
}

async function clickByText(container: HTMLElement, text: string) {
  await act(async () => {
    buttonByText(container, text).dispatchEvent(
      new MouseEvent("click", { bubbles: true }),
    );
    await Promise.resolve();
  });
}
