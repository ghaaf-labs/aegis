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
    expect(text).toContain("Base → Arc");
    expect(text).toContain("Wallet cash");
    expect(text).toContain("Target mix");
    expect(text).not.toContain("Bridge Arc → Base");
    expect(text).not.toContain("Arc → Base");

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
    expect(text).toContain("Sold positions");
    expect(text).toContain("changed to USDC");
    expect(text).toContain("Plan amount");
    expect(text).toContain("One approval executes the planned route");
    expect(text).not.toContain("One approval executes the Base legs");
    expect(text).not.toContain("One approval executes the Arc legs");
    expect(text).not.toContain("BASE WALLET");

    act(() => root.unmount());
  });

  it("hides internal route blocker details from the approval copy", () => {
    const plan: RebalancePlanResponse = {
      rebalanceId: "rebalance-3",
      decisionId: "decision-3",
      executionMode: "real",
      totalLegs: 1,
      legs: [
        {
          legIndex: 0,
          kind: "local_swap",
          srcChain: "base",
          destChain: "base",
          srcSymbol: "USDC",
          destSymbol: "BTC",
          amountUsdc: 100,
        },
      ],
    };

    const { root, container } = render(
      <ApprovalModal
        open
        plan={plan}
        estimatedFeeUsdc={0.01}
        approvalSafety={{
          approvable: false,
          code: "EXECUTION_UNAVAILABLE",
          message:
            "internal: enable the missing adapter or cargo feature before approving",
          missingCapabilities: [
            {
              code: "LOCAL_SWAP_ADAPTER",
              label: "Swap route not ready",
              detail: "internal adapter detail",
            },
          ],
        }}
        onApproved={() => {}}
        onClose={() => {}}
      />,
    );

    const text = container.textContent ?? "";
    expect(text).toContain("Route not ready");
    expect(text).toContain("one selected route is not ready");
    expect(text).toContain("Swap route not ready");
    expect(text).not.toContain("cargo feature");
    expect(text).not.toContain("adapter detail");
    expect(text).not.toContain("Execution unavailable");

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
