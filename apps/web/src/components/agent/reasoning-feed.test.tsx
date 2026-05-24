import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { AgentReasoningFeed } from "./reasoning-feed";
import { usePortfolioStore } from "@/stores/portfolio";
import type { AgentDecision, Portfolio } from "@/types";

const PORTFOLIO: Portfolio = {
  id: "portfolio-1",
  userId: "user-1",
  name: "Core treasury",
  totalValueUsd: 1200,
  totalPnlUsd: 0,
  totalPnlPct: 0,
  allocations: [],
  riskScore: 42,
  goal: null,
  createdAt: "2026-05-20T00:00:00Z",
  updatedAt: "2026-05-20T00:00:00Z",
};

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  usePortfolioStore.getState().resetSession();
});

describe("<AgentReasoningFeed />", () => {
  it("renders malformed historical trade rows as review items instead of crashing", () => {
    const malformedDecision = {
      id: "decision-1",
      portfolioId: PORTFOLIO.id,
      reasoning: "",
      recommendation: {
        summary: "Malformed backend row",
        trades: [
          {
            symbol: "ETH",
            valueUsd: 100,
          },
          {
            action: "rotate",
            reason: "unsupported planner action",
          },
        ],
        expectedImpact: {
          riskDelta: 0,
          diversificationScore: 0,
        },
      },
      confidence: Number.NaN,
      triggeredBy: "unknown_trigger",
      createdAt: "2026-05-20T00:00:00Z",
    } as unknown as AgentDecision;

    usePortfolioStore.setState({
      portfolios: [PORTFOLIO],
      portfoliosLoaded: true,
      activePortfolioId: PORTFOLIO.id,
      decisions: [malformedDecision],
      unifiedUsdc: 0,
    });

    const { root, container } = render(<AgentReasoningFeed />);
    let text = container.textContent ?? "";

    expect(
      container.querySelector('button[aria-label="Refresh decisions"]'),
    ).toBeTruthy();
    expect(text).toContain("User Review");
    expect(text).toContain("0%");
    expect(text).toContain("Review");
    expect(text).toContain("ETH");
    expect(text).toContain("UNKNOWN");
    expect(text).toContain("Needs review");
    expect(text).toContain("No funds move until you approve.");
    expect(text).not.toContain("No reasoning was returned with this decision.");

    const fullAuditButton = Array.from(
      container.querySelectorAll("button"),
    ).find((button) => button.textContent?.includes("History"));
    expect(fullAuditButton).toBeTruthy();
    act(() => {
      fullAuditButton!.dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true }),
      );
    });

    text = container.textContent ?? "";
    expect(text).toContain("No reasoning was returned with this decision.");

    act(() => root.unmount());
  });

  it("uses legacy usdValue trades when calculating review state and row amounts", () => {
    const legacyDecision = {
      id: "decision-legacy",
      portfolioId: PORTFOLIO.id,
      reasoning: "Deterministic planner output from an older row.",
      recommendation: {
        summary: "Deploy stale idle cash",
        trades: [
          {
            symbol: "USYC",
            action: "buy",
            usdValue: 250,
            reason: "legacy deterministic planner amount",
          },
        ],
        expectedImpact: {
          riskDelta: 0,
          diversificationScore: 0,
        },
      },
      confidence: 0.72,
      triggeredBy: "user_request",
      createdAt: "2026-05-20T00:00:00Z",
    } as unknown as AgentDecision;

    usePortfolioStore.setState({
      portfolios: [PORTFOLIO],
      portfoliosLoaded: true,
      activePortfolioId: PORTFOLIO.id,
      decisions: [legacyDecision],
      unifiedUsdc: 0,
    });

    const { root, container } = render(<AgentReasoningFeed />);
    let text = container.textContent ?? "";

    expect(text).toContain("History (1)");
    expect(text).toContain("1 older or stale row is in History");
    expect(text).toContain("No current plan");

    const fullAuditButton = Array.from(
      container.querySelectorAll("button"),
    ).find((button) => button.textContent?.includes("History"));
    expect(fullAuditButton).toBeTruthy();
    act(() => {
      fullAuditButton!.dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true }),
      );
    });

    text = container.textContent ?? "";
    expect(text).toContain("USYC");
    expect(text).toContain("$250.00");
    expect(text).toContain("Needs fresh plan");

    act(() => root.unmount());
  });

  it("keeps no-move monitor checks collapsed in Current", () => {
    const moveDecision = {
      id: "decision-move",
      portfolioId: PORTFOLIO.id,
      reasoning: "Move idle cash into the target allocation.",
      recommendation: {
        summary: "Move idle cash",
        trades: [
          {
            symbol: "ETH",
            action: "buy",
            valueUsd: 100,
            reason: "Base route",
          },
        ],
        expectedImpact: {
          riskDelta: 0,
          diversificationScore: 0,
        },
      },
      confidence: 0.91,
      triggeredBy: "user_request",
      createdAt: "2026-05-20T00:00:00Z",
    } as unknown as AgentDecision;
    const holdDecision = {
      ...moveDecision,
      id: "decision-hold",
      reasoning: "Portfolio remains inside target bands.",
      recommendation: {
        summary: "No move needed",
        trades: [],
        expectedImpact: {
          riskDelta: 0,
          diversificationScore: 0,
        },
      },
      confidence: 0.86,
    } as unknown as AgentDecision;

    usePortfolioStore.setState({
      portfolios: [PORTFOLIO],
      portfoliosLoaded: true,
      activePortfolioId: PORTFOLIO.id,
      decisions: [moveDecision, holdDecision],
      unifiedUsdc: 500,
    });

    const { root, container } = render(<AgentReasoningFeed />);
    const text = container.textContent ?? "";

    expect(text).toContain("Current (1)");
    expect(text).toContain("Move $100.00 to ETH");
    expect(text).toContain("+ 1 monitor check reports no movement needed.");
    expect(text).not.toContain("Portfolio remains inside target bands.");

    act(() => root.unmount());
  });

  it("renders many future trade legs with arbitrary token symbols", () => {
    const trades = Array.from({ length: 18 }, (_, i) => ({
      symbol: `TOKEN${i + 1}`,
      action: "buy",
      valueUsd: 10 + i,
      reason: i % 2 === 0 ? "Base route" : "Arc route",
    }));
    const largeDecision = {
      id: "decision-many-legs",
      portfolioId: PORTFOLIO.id,
      reasoning: "Split across a wider token set.",
      recommendation: {
        summary: "Move into a wider token basket",
        trades,
        expectedImpact: {
          riskDelta: 0,
          diversificationScore: 0,
        },
      },
      confidence: 0.89,
      triggeredBy: "user_request",
      createdAt: "2026-05-20T00:00:00Z",
      modelSlug: "provider/future-model-with-a-long-routing-slug",
    } as unknown as AgentDecision;

    usePortfolioStore.setState({
      portfolios: [PORTFOLIO],
      portfoliosLoaded: true,
      activePortfolioId: PORTFOLIO.id,
      decisions: [largeDecision],
      unifiedUsdc: 1_000,
    });

    const { root, container } = render(<AgentReasoningFeed />);
    const text = container.textContent ?? "";

    expect(text).toContain("18 legs");
    expect(text).toContain(
      "Move $333.00 to TOKEN1 / TOKEN2 / TOKEN3 / TOKEN4 / TOKEN5 + 13 more",
    );
    expect(text).toContain("TOKEN18");
    expect(text).toContain("$27.00");
    expect(text).toContain("provider/future-model-with-a-long-routing-slug");
    expect(container.querySelector("table")).toBeTruthy();
    expect(container.querySelectorAll("tbody tr")).toHaveLength(18);

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
