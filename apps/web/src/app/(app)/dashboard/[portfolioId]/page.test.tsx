import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { AgentDecision, Portfolio } from "@/types";
import { usePortfolioStore } from "@/stores/portfolio";

// next/navigation: land on /dashboard/p1?designing=1 with a stable router.
const push = vi.fn();
const replace = vi.fn();
vi.mock("next/navigation", () => ({
  useParams: () => ({ portfolioId: "p1" }),
  useSearchParams: () => new URLSearchParams("designing=1"),
  useRouter: () => ({ push, replace }),
}));

// Render framer-motion elements as plain divs (children only).
vi.mock("framer-motion", () => ({
  motion: new Proxy(
    {},
    {
      get:
        () =>
        ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
    },
  ),
}));

// Stub icons. NOTE: must be an explicit object, not a bare `Proxy` module — a
// Proxy whose `get` returns a function for ANY key makes `module.then` a
// function, so vitest's ESM interop treats the module as a thenable and awaits
// it forever (hangs collection).
vi.mock("lucide-react", () => ({
  CircleAlert: () => null,
  Loader2: () => null,
  LockKeyhole: () => null,
  Rocket: () => null,
  Sparkles: () => null,
}));

// Stub the heavy presentational children — this test is about the
// `?designing=1` handoff effect, not their rendering.
vi.mock("@/components/dashboard/allocation-chart", () => ({
  AllocationChart: () => null,
}));
vi.mock("@/components/dashboard/asset-table", () => ({
  AssetTable: () => null,
}));
vi.mock("@/components/agent/reasoning-feed", () => ({
  AgentReasoningFeed: () => null,
}));
vi.mock("@/components/dashboard/performance-chart", () => ({
  PerformanceChart: () => null,
}));
vi.mock("@/components/dashboard/market-overview", () => ({
  MarketOverview: () => null,
}));
vi.mock("@/components/dashboard/trustability-card", () => ({
  TrustabilityCard: () => null,
}));
vi.mock("@/components/dashboard/asset-control-tower", () => ({
  AssetControlTower: () => <div data-testid="asset-control-tower" />,
}));
vi.mock("@/components/dashboard/route-stack-matrix", () => ({
  RouteStackMatrix: () => <div data-testid="route-stack-matrix" />,
}));
vi.mock("@/components/dashboard/idle-cash-card", () => ({
  IdleCashCard: () => null,
}));
vi.mock("@/components/wallet/faucet-button", () => ({
  FaucetButton: () => null,
}));
vi.mock("@/components/rebalance/approval-modal", () => ({
  ApprovalModal: () => null,
}));
// Expose the proposal modal's open state so we can assert Gate 1 opened.
vi.mock("@/components/agent/allocation-proposal-modal", () => ({
  AllocationProposalModal: ({ open }: { open: boolean }) =>
    open ? <div data-testid="proposal-modal-open" /> : null,
}));
vi.mock("@aegis/ui", () => ({
  BrutalButton: ({ children }: { children?: React.ReactNode }) => (
    <button type="button">{children}</button>
  ),
  Skeleton: () => <div data-testid="skeleton" />,
}));

// Keep derived state simple + safe: no agent target yet (so the designing flow
// runs), zeroed metrics.
vi.mock("@/components/dashboard/target-allocations", () => ({
  targetAllocationsForPortfolio: () => [],
}));
vi.mock("@/lib/portfolio-values", () => ({
  derivePortfolioPositionMetrics: () => ({ investedUsd: 0, maxDriftPct: 0 }),
}));
vi.mock("@/lib/cash-model", () => ({
  deriveCashSplit: () => ({ usdcTargetWeight: 100, deployableUsd: 0 }),
}));
vi.mock("@/lib/dashboard-balance-model", () => ({
  deriveDashboardBalanceModel: () => ({
    investedUsd: 0,
    hasInvestedPositions: false,
    hasAgentTarget: false,
    deployableUsd: 0,
    hasIdleCash: false,
    maxTargetDriftPct: 0,
    hasReviewableDrift: false,
  }),
}));
vi.mock("@/lib/utils", () => ({ formatCurrency: (n: number) => `$${n}` }));
vi.mock("@/lib/proposal-dismissal", () => ({
  isProposalDismissed: () => false,
  dismissProposal: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  agentApi: { proposeAllocation: vi.fn(), decisionById: vi.fn() },
  rebalanceApi: {
    plan: vi.fn(),
    get: vi.fn(),
    history: vi.fn(() => Promise.resolve([])),
  },
  userAgentApi: { autoPilot: vi.fn() },
}));

import PortfolioDashboardPage from "./page";
import { agentApi, userAgentApi } from "@/lib/api";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
  usePortfolioStore.getState().resetSession();
  window.localStorage.clear();
});

describe("<PortfolioDashboardPage /> ?designing=1 handoff", () => {
  it("fires the allocator once, shows the designing state, then opens Gate 1", async () => {
    vi.mocked(userAgentApi.autoPilot).mockResolvedValue({
      pausedAt: null,
      autoPilotEnabled: false,
    });
    // Async contract: proposeAllocation enqueues and returns a `queued`
    // placeholder; the dashboard then polls decisionById until it is `ready`.
    let resolveEnqueue!: (d: AgentDecision) => void;
    const enqueuePromise = new Promise<AgentDecision>((resolve) => {
      resolveEnqueue = resolve;
    });
    vi.mocked(agentApi.proposeAllocation).mockReturnValue(enqueuePromise);
    vi.mocked(agentApi.decisionById).mockResolvedValue(
      proposalDecision("ready"),
    );

    usePortfolioStore.getState().setPortfolios([portfolio("p1")]);
    usePortfolioStore.getState().setActivePortfolio("p1");
    usePortfolioStore.getState().setActivePortfolioDetailStatus("p1", "ready");
    usePortfolioStore.getState().setDecisionsStatus("p1", "ready");
    usePortfolioStore.getState().setMarketSnapshotStatus("ready");
    usePortfolioStore.getState().setGatewayBalanceStatus("ready");

    const { container, root } = render(<PortfolioDashboardPage />);
    await flushEffects();

    // Generation kicked off exactly once, and the designing banner is shown
    // while the enqueue is pending (modal not open yet).
    expect(agentApi.proposeAllocation).toHaveBeenCalledTimes(1);
    expect(agentApi.proposeAllocation).toHaveBeenCalledWith("p1");
    expect(container.textContent).toContain("designing your allocation");
    expect(
      container.querySelector('[data-testid="proposal-modal-open"]'),
    ).toBeNull();

    // Enqueue resolves (queued) → the poll fetches a `ready` decision → Gate 1
    // opens, designing banner clears.
    await act(async () => {
      resolveEnqueue(proposalDecision("queued"));
      await flushMicrotasks();
    });
    expect(agentApi.decisionById).toHaveBeenCalledWith("decision-1");
    expect(
      container.querySelector('[data-testid="proposal-modal-open"]'),
    ).not.toBeNull();

    // Still exactly one call — the ref guard prevents a re-render double-fire.
    expect(agentApi.proposeAllocation).toHaveBeenCalledTimes(1);

    act(() => root.unmount());
  });

  it("clears a stale designing handoff on browser refresh", async () => {
    const navigationSpy = vi
      .spyOn(performance, "getEntriesByType")
      .mockImplementation((type) =>
        type === "navigation"
          ? ([{ type: "reload" }] as unknown as PerformanceEntryList)
          : [],
      );
    vi.mocked(userAgentApi.autoPilot).mockResolvedValue({
      pausedAt: null,
      autoPilotEnabled: false,
    });
    vi.mocked(agentApi.proposeAllocation).mockRejectedValue(
      new Error("should not run"),
    );

    usePortfolioStore.getState().setPortfolios([portfolio("p1")]);
    usePortfolioStore.getState().setActivePortfolio("p1");
    usePortfolioStore.getState().setActivePortfolioDetailStatus("p1", "ready");
    usePortfolioStore.getState().setDecisionsStatus("p1", "ready");
    usePortfolioStore.getState().setMarketSnapshotStatus("ready");
    usePortfolioStore.getState().setGatewayBalanceStatus("ready");

    const { container, root } = render(<PortfolioDashboardPage />);
    await flushEffects();

    expect(agentApi.proposeAllocation).not.toHaveBeenCalled();
    expect(replace).toHaveBeenCalledWith("/dashboard/p1", { scroll: false });
    expect(container.textContent).not.toContain("designing your allocation");
    expect(container.textContent).not.toContain("could not finish designing");

    act(() => root.unmount());
    navigationSpy.mockRestore();
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
    await flushMicrotasks();
  });
}

async function flushMicrotasks() {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function proposalDecision(status: "queued" | "ready" = "ready"): AgentDecision {
  return {
    id: "decision-1",
    portfolioId: "p1",
    reasoning: "Balanced mix.",
    recommendation: {
      summary: "",
      trades: [],
      expectedImpact: { riskDelta: 0, diversificationScore: 0 },
    },
    confidence: 0.8,
    triggeredBy: "user_request",
    kind: "allocation_proposal",
    recommendedAllocation: { USDC: 60, cbBTC: 40 },
    createdAt: new Date().toISOString(),
    status,
  };
}

function portfolio(id: string, name = "Agent-managed portfolio"): Portfolio {
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
