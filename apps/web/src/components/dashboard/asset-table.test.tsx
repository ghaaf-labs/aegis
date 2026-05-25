import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { MarketSnapshot, Portfolio } from "@/types";
import type { DashboardBalanceModel } from "@/lib/dashboard-balance-model";
import { usePortfolioStore } from "@/stores/portfolio";
import { AssetTable } from "./asset-table";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
  window.localStorage.clear();
  vi.clearAllMocks();
});

describe("<AssetTable />", () => {
  it("runs the review handler from the after-approval preview", () => {
    const onReviewPlan = vi.fn();
    seedAfterApprovalState();

    const { container, root } = render(
      <AssetTable onReviewPlan={onReviewPlan} />,
    );

    const button = [...container.querySelectorAll("button")].find((candidate) =>
      candidate.textContent?.includes("Review plan"),
    );
    expect(button).toBeTruthy();
    expect(button?.className).toContain("min-h-11");
    expect(button?.className).toContain("w-full");
    expect(button?.className).toContain("sm:w-auto");
    const targetGrid = container.querySelector('div[class*="auto-rows-fr"]');
    expect(targetGrid?.className).toContain("xl:grid-cols-3");

    act(() => {
      button?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(onReviewPlan).toHaveBeenCalledTimes(1);

    act(() => root.unmount());
  });

  it("renders invested holdings with compact mobile metrics", () => {
    seedInvestedState();

    const { container, root } = render(<AssetTable />);
    const mobileList = container.querySelector('div[class*="lg:hidden"]');

    expect(container.textContent).toContain("Current Holdings");
    expect(mobileList?.textContent).toContain("BTC");
    expect(mobileList?.textContent).toContain("Price");
    expect(mobileList?.textContent).toContain("24h");
    expect(mobileList?.textContent).toContain("Units");
    expect(mobileList?.textContent).toContain("Weight");
    expect(mobileList?.textContent).toContain("$32,000.00");
    expect(container.querySelector("table")?.className).toContain("lg:table");
    expect(
      [...container.querySelectorAll("th")].find(
        (header) => header.textContent === "24h",
      )?.className,
    ).toContain("xl:table-cell");

    act(() => root.unmount());
  });

  it("uses dashboard balance model values for current holdings", () => {
    seedModelBackedState();

    const { container, root } = render(
      <AssetTable model={makeBalanceModel()} />,
    );

    expect(container.textContent).toContain("Current Exposure");
    expect(container.textContent).toContain("ETH");
    expect(container.textContent).toContain("LINK");
    expect(container.textContent).toContain("USDC");
    expect(container.textContent).toContain("$161.77");
    expect(container.textContent).toContain("$24.21");
    expect(container.textContent).toContain("$260.35");
    expect(container.textContent).toContain("wallet");
    expect(
      [...container.querySelectorAll("th")].some(
        (header) => header.textContent === "Units",
      ),
    ).toBe(true);

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

function seedAfterApprovalState() {
  const portfolio = makePortfolio();
  const store = usePortfolioStore.getState();
  act(() => {
    store.setPortfolios([portfolio]);
    store.setActivePortfolio(portfolio.id);
    store.setUnifiedUsdc(100);
    store.setUnifiedEurc(0);
    store.setGatewayBalanceStatus("ready");
  });
}

function seedInvestedState() {
  const now = new Date().toISOString();
  const portfolio: Portfolio = {
    id: "portfolio-2",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 30_000,
    totalPnlUsd: 1_000,
    totalPnlPct: 3.33,
    allocations: [allocation("BTC", 60, 1, 30_000)],
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
  const snapshot: MarketSnapshot = {
    id: "snapshot-1",
    capturedAt: now,
    fearGreedIndex: 50,
    totalMarketCapUsd: 1_000_000_000,
    btcDominance: 52,
    assets: [
      {
        symbol: "BTC",
        priceUsd: 32_000,
        change24h: 3.2,
        change7d: 7,
        marketCap: 600_000_000,
        volume24h: 10_000_000,
        updatedAt: now,
      },
    ],
  };
  const store = usePortfolioStore.getState();
  act(() => {
    store.setPortfolios([portfolio]);
    store.setActivePortfolio(portfolio.id);
    store.setMarketSnapshot(snapshot);
    store.setGatewayBalanceStatus("ready");
  });
}

function seedModelBackedState() {
  const now = new Date().toISOString();
  const portfolio: Portfolio = {
    id: "portfolio-model",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 185.98,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [
      allocation("ETH", 60, 0, 0),
      allocation("LINK", 10, 0, 0),
      allocation("USDC", 30, 0, 0),
    ],
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
  const snapshot: MarketSnapshot = {
    id: "snapshot-model",
    capturedAt: now,
    fearGreedIndex: 50,
    totalMarketCapUsd: 1_000_000_000,
    btcDominance: 52,
    assets: [
      {
        symbol: "ETH",
        priceUsd: 2_098.16,
        change24h: 0,
        change7d: 0,
        marketCap: 1,
        volume24h: 1,
        updatedAt: now,
      },
      {
        symbol: "LINK",
        priceUsd: 14,
        change24h: 0,
        change7d: 0,
        marketCap: 1,
        volume24h: 1,
        updatedAt: now,
      },
    ],
  };
  const store = usePortfolioStore.getState();
  act(() => {
    store.setPortfolios([portfolio]);
    store.setActivePortfolio(portfolio.id);
    store.setMarketSnapshot(snapshot);
    store.setUnifiedUsdc(260.35);
    store.setGatewayBalanceStatus("ready");
  });
}

function makeBalanceModel(): DashboardBalanceModel {
  return {
    netWorthUsd: 446.33,
    investedUsd: 185.98,
    walletValueUsd: 260.35,
    reserveUsd: 260.35,
    reservePct: 30,
    deployableUsd: 0,
    unifiedUsdc: 260.35,
    unifiedEurc: 0,
    eurcUsd: 1,
    hasIdleCash: true,
    hasAgentTarget: true,
    hasInvestedPositions: true,
    hasReviewableDrift: false,
    maxTargetDriftPct: 0,
    walletBalanceLoading: false,
    walletBalanceUnavailable: false,
    gatewayBalanceError: null,
    gatewayBalanceUpdatedAt: Date.now(),
    status: {
      label: "Monitoring",
      detail: "No cash is queued.",
      tone: "pnl",
    },
    tokens: [
      {
        symbol: "USDC",
        walletUsd: 260.35,
        investedUsd: 0,
        totalUsd: 260.35,
        targetWeight: 30,
        weightPct: 58.33,
      },
      {
        symbol: "ETH",
        walletUsd: 161.77,
        investedUsd: 0,
        totalUsd: 161.77,
        targetWeight: 60,
        weightPct: 36.24,
      },
      {
        symbol: "LINK",
        walletUsd: 24.21,
        investedUsd: 0,
        totalUsd: 24.21,
        targetWeight: 10,
        weightPct: 5.42,
      },
    ],
    chains: [],
    matrixRows: [],
    matrixTotals: [],
    matrixTotalUsd: 446.33,
    tokenCount: 3,
    chainCount: 0,
    addressCount: 0,
  };
}

function makePortfolio(): Portfolio {
  const now = new Date().toISOString();
  return {
    id: "portfolio-1",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 0,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [allocation("cbBTC", 60), allocation("USDC", 40)],
    riskScore: 0,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
}

function allocation(
  symbol: string,
  targetWeight: number,
  quantity = 0,
  valueUsd = 0,
) {
  return {
    assetId: `asset-${symbol}`,
    symbol,
    quantity,
    targetWeight,
    currentWeight: 0,
    valueUsd,
  };
}
