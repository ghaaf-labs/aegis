import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act, type ReactElement, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { MarketSnapshot, Portfolio, WalletInfo } from "@/types";
import { usePortfolioStore } from "@/stores/portfolio";
import PortfolioPage from "./page";

vi.mock("@/components/dashboard/asset-table", () => ({
  AssetTable: ({ model }: { model?: ModelProbe }) => (
    <div data-testid="asset-table">
      {model?.tokens
        .map((token) => {
          return `${token.symbol}:${token.totalUsd.toFixed(2)}:${token.weightPct.toFixed(1)}`;
        })
        .join("|")}
    </div>
  ),
}));

vi.mock("@/components/dashboard/allocation-chart", () => ({
  AllocationChart: ({ model }: { model?: ModelProbe }) => (
    <div data-testid="allocation-chart">{model?.netWorthUsd.toFixed(2)}</div>
  ),
}));

vi.mock("@/components/portfolio/risk-score-card", () => ({
  RiskScoreCard: ({ model }: { model?: ModelProbe }) => (
    <div data-testid="risk-score">{String(model?.hasInvestedPositions)}</div>
  ),
}));

vi.mock("@/components/portfolio/rebalance-modal", () => ({
  RebalanceModal: () => null,
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
    ...props
  }: {
    children: ReactNode;
    href: string;
  }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("@/lib/api", () => ({
  agentApi: {
    proposeAllocation: vi.fn(),
  },
}));

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

describe("<PortfolioPage />", () => {
  it("renders wallet token exposure from the reconciled balance model", () => {
    seedPortfolioPageState();

    const { container, root } = render(<PortfolioPage />);

    expect(container.textContent).toContain("Net worth");
    expect(container.textContent).toContain("$428.75");
    expect(container.textContent).toContain("$25.75");
    expect(screenText(container, "asset-table")).toContain("ETH:378.80:88.3");
    expect(screenText(container, "asset-table")).toContain("USDC:25.75:6.0");
    expect(screenText(container, "asset-table")).toContain("LINK:24.20:5.6");
    expect(screenText(container, "allocation-chart")).toBe("428.75");
    expect(screenText(container, "risk-score")).toBe("true");

    act(() => root.unmount());
  });
});

type ModelProbe = {
  netWorthUsd: number;
  hasInvestedPositions: boolean;
  tokens: Array<{
    symbol: string;
    totalUsd: number;
    weightPct: number;
  }>;
};

function render(element: ReactElement): {
  container: HTMLDivElement;
  root: Root;
} {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() => root.render(element));
  return { container, root };
}

function screenText(container: HTMLElement, testId: string) {
  return (
    container.querySelector(`[data-testid="${testId}"]`)?.textContent ?? ""
  );
}

function seedPortfolioPageState() {
  const now = new Date().toISOString();
  const portfolio: Portfolio = {
    id: "portfolio-1",
    userId: "user-1",
    name: "Agent-managed portfolio",
    totalValueUsd: 378.74,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [
      allocation("ETH", 30, 0.1802, 378.74),
      allocation("USDC", 70, 0, 0),
    ],
    riskScore: 50,
    goal: null,
    createdAt: now,
    updatedAt: now,
  };
  const wallet: WalletInfo = {
    walletId: "wallet-1",
    arcAddress: "0xarc",
    baseAddress: "0xbase",
    networks: [
      {
        blockchain: "BASE-SEPOLIA",
        walletId: "wallet-1",
        address: "0xbase",
        accountType: "EOA",
        state: "LIVE",
      },
    ],
    createdAt: now,
  };
  const snapshot: MarketSnapshot = {
    id: "snapshot-1",
    capturedAt: now,
    fearGreedIndex: 50,
    totalMarketCapUsd: 0,
    btcDominance: 0,
    assets: [marketAsset("ETH", 2102.09), marketAsset("LINK", 9.46)],
  };
  const store = usePortfolioStore.getState();
  act(() => {
    store.setPortfolios([portfolio]);
    store.setActivePortfolio(portfolio.id);
    store.setWallet(wallet);
    store.setMarketSnapshot(snapshot);
    store.setUnifiedUsdc(25.75);
    store.setUnifiedEurc(0);
    store.setPerChain({ base: 25.75 }, {}, Date.now(), {
      base: { ETH: 0.1802, LINK: 2.5586 },
    });
    store.setGatewayBalanceStatus("ready");
  });
}

function allocation(
  symbol: string,
  targetWeight: number,
  quantity: number,
  valueUsd: number,
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

function marketAsset(symbol: string, priceUsd: number) {
  return {
    symbol,
    priceUsd,
    change24h: 0,
    change7d: 0,
    marketCap: 0,
    volume24h: 0,
    updatedAt: new Date().toISOString(),
  };
}
