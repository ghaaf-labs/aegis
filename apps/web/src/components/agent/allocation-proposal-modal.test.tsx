import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { AgentDecision } from "@/types";
import { AllocationProposalModal } from "./allocation-proposal-modal";

const apiMock = vi.hoisted(() => ({
  approveAllocation: vi.fn(),
  getPortfolio: vi.fn(),
  proposeAllocation: vi.fn(),
}));

const pollMock = vi.hoisted(() => ({
  pollDecisionReady: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  agentApi: {
    approveAllocation: apiMock.approveAllocation,
    proposeAllocation: apiMock.proposeAllocation,
  },
  portfolioApi: {
    get: apiMock.getPortfolio,
  },
}));

vi.mock("@/lib/decision-poll", () => ({
  pollDecisionReady: pollMock.pollDecisionReady,
}));

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
});

describe("<AllocationProposalModal />", () => {
  it("shows immediate risk-dial progress while a re-propose job runs", () => {
    apiMock.proposeAllocation.mockReturnValue(new Promise(() => {}));

    const { container, root } = renderModal();

    clickButton(container, "Conservative");

    expect(container.textContent).toContain(
      "Designing conservative allocation",
    );
    expect(container.textContent).toContain("Designing");
    expect(apiMock.proposeAllocation).toHaveBeenCalledWith(
      "portfolio-1",
      "conservative",
    );

    act(() => root.unmount());
  });

  it("polls the ready re-propose result before replacing the visible plan", async () => {
    apiMock.proposeAllocation.mockResolvedValue({
      ...decisionFixture(),
      id: "queued-decision",
      status: "queued",
    });
    pollMock.pollDecisionReady.mockResolvedValue({
      ...decisionFixture(),
      id: "ready-decision",
      reasoning: "Ready conservative plan.",
      recommendedAllocation: { USDC: 100 },
      status: "ready",
    });

    const { container, root } = renderModal();

    clickButton(container, "Conservative");
    await flushEffects();

    expect(pollMock.pollDecisionReady).toHaveBeenCalledWith(
      "queued-decision",
      expect.any(Function),
    );
    expect(container.textContent).toContain("Ready conservative plan.");
    expect(container.textContent).toContain("USDC");

    act(() => root.unmount());
  });

  it("approves and advances to review from one click", async () => {
    const approved = deferred<void>();
    const onApproved = vi.fn().mockResolvedValue(undefined);
    apiMock.approveAllocation.mockReturnValue(approved.promise);
    apiMock.getPortfolio.mockRejectedValue(new Error("store will refresh"));

    const { container, root } = renderModal({ onApproved });

    clickButton(container, "Approve allocation");

    expect(container.textContent).toContain("Approving allocation");
    expect(container.textContent).not.toContain("Review deployment plan");

    await act(async () => {
      approved.resolve();
      await approved.promise;
      await Promise.resolve();
    });

    expect(onApproved).toHaveBeenCalledTimes(1);

    act(() => root.unmount());
  });
});

function renderModal({
  decision = decisionFixture(),
  onApproved = vi.fn(),
}: {
  decision?: AgentDecision;
  onApproved?: () => void | Promise<void>;
} = {}): {
  container: HTMLDivElement;
  root: Root;
} {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <AllocationProposalModal
        open
        portfolioId="portfolio-1"
        decision={decision}
        onClose={() => {}}
        onApproved={onApproved}
      />,
    ),
  );
  return { container, root };
}

function clickButton(container: HTMLElement, label: string) {
  const button = [...container.querySelectorAll("button")].find((candidate) =>
    candidate.textContent?.includes(label),
  );
  expect(button).toBeTruthy();
  act(() => {
    button?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
}

function decisionFixture(): AgentDecision {
  const now = "2026-05-24T18:00:00.000Z";
  return {
    id: "decision-1",
    portfolioId: "portfolio-1",
    reasoning: "Initial allocation plan.",
    recommendation: {
      summary: "Allocate idle cash.",
      trades: [],
      expectedImpact: {
        diversificationScore: 80,
        riskDelta: 0,
      },
    },
    confidence: 0.85,
    rawConfidence: 0.85,
    calibratedConfidence: 0.85,
    triggeredBy: "user_request",
    createdAt: now,
    kind: "allocation_proposal",
    modelSlug: "google/gemini-3.5-flash-20260519",
    regime: "neutral",
    recommendedAllocation: {
      cbBTC: 30,
      ETH: 25,
      USDC: 20,
      SOL: 15,
      LINK: 5,
      UNI: 5,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, reject, resolve };
}

async function flushEffects() {
  await act(async () => {
    for (let i = 0; i < 6; i += 1) {
      await Promise.resolve();
    }
  });
}
