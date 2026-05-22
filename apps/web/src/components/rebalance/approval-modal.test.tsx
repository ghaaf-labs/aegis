import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { ApprovalModal } from "./approval-modal";
import type { RebalancePlanResponse } from "@/lib/api";

vi.mock("@/components/rebalance/backtest-preview", () => ({
  BacktestPreview: () => null,
}));

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("<ApprovalModal />", () => {
  it("labels cross-chain route direction from the actual burn leg", () => {
    const plan: RebalancePlanResponse = {
      rebalanceId: "rebalance-1",
      decisionId: "decision-1",
      executionMode: "real",
      totalLegs: 2,
      legs: [
        {
          legIndex: 0,
          kind: "cross_chain_burn",
          srcChain: "base",
          destChain: "arc",
          srcSymbol: "USDC",
          destSymbol: "BTC",
          amountUsdc: 50,
        },
        {
          legIndex: 1,
          kind: "cross_chain_mint",
          srcChain: "base",
          destChain: "arc",
          srcSymbol: "USDC",
          destSymbol: "USDC",
          amountUsdc: 50,
        },
      ],
    };

    const { root, container } = render(
      <ApprovalModal
        open
        plan={plan}
        estimatedFeeUsdc={0.01}
        onApproved={() => {}}
        onClose={() => {}}
      />,
    );

    const text = container.textContent ?? "";
    expect(text).toContain("Bridge Base → Arc");
    expect(text).toContain("BASE → ARC");
    expect(text).toContain("BASE SOURCE");
    expect(text).toContain("ARC TARGET");
    expect(text).not.toContain("Bridge Arc → Base");
    expect(text).not.toContain("ARC SOURCE");
    expect(text).not.toContain("BASE TARGET");

    act(() => root.unmount());
  });

  it("labels sell-and-buy rebalances as position turnover, not wallet cash", () => {
    const plan: RebalancePlanResponse = {
      rebalanceId: "rebalance-2",
      decisionId: "decision-2",
      executionMode: "real",
      totalLegs: 2,
      legs: [
        {
          legIndex: 0,
          kind: "local_swap",
          srcChain: "base",
          destChain: "base",
          srcSymbol: "BTC",
          destSymbol: "USDC",
          amountUsdc: 600,
        },
        {
          legIndex: 1,
          kind: "local_swap",
          srcChain: "base",
          destChain: "base",
          srcSymbol: "USDC",
          destSymbol: "ETH",
          amountUsdc: 600,
        },
      ],
    };

    const { root, container } = render(
      <ApprovalModal
        open
        plan={plan}
        estimatedFeeUsdc={0.01}
        onApproved={() => {}}
        onClose={() => {}}
      />,
    );

    const text = container.textContent ?? "";
    expect(text).toContain("Rebalance $600.00 from overweight positions");
    expect(text).toContain("BASE SOLD");
    expect(text).toContain("positions to USDC");
    expect(text).toContain("Gross leg notional");
    expect(text).toContain("One approval executes the planned route");
    expect(text).not.toContain("One approval executes the Base legs");
    expect(text).not.toContain("One approval executes the Arc legs");
    expect(text).not.toContain("BASE WALLET");

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
