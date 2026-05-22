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
    const text = container.textContent ?? "";

    expect(text).toContain("Manual");
    expect(text).toContain("0%");
    expect(text).toContain("REVIEW");
    expect(text).toContain("ETH");
    expect(text).toContain("UNKNOWN");
    expect(text).toContain("Malformed historical trade row");
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

    expect(text).toContain("Full audit · 1");
    expect(text).toContain("1 historical, rejected, or cash-mismatched row");
    expect(text).toContain("No current executable guidance");

    const fullAuditButton = Array.from(
      container.querySelectorAll("button"),
    ).find((button) => button.textContent?.includes("Full audit"));
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
