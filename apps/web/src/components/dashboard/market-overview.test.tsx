import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { MarketSnapshot } from "@/types";
import { usePortfolioStore } from "@/stores/portfolio";
import { MarketOverview } from "./market-overview";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
});

describe("<MarketOverview />", () => {
  it("renders the split-gauge market summary with movers and macro context", () => {
    usePortfolioStore.getState().setMarketSnapshot(snapshot());

    const { container, root } = render(<MarketOverview />);
    const text = container.textContent ?? "";

    expect(text).toContain("Fear & Greed Index");
    expect(text).toContain("25");
    expect(text).toContain("Fear");
    expect(text).toContain("Top movers (24h)");
    expect(text).toContain("Market cap");
    expect(text).toContain("BTC dominance");
    expect(text).toContain("24h volume");
    expect(text.indexOf("BTC")).toBeLessThan(text.indexOf("ETH"));
    expect(text).toContain("$2.6T");

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

function snapshot(): MarketSnapshot {
  const now = new Date().toISOString();
  return {
    id: "market-1",
    fearGreedIndex: 25,
    totalMarketCapUsd: 2_600_000_000_000,
    btcDominance: 58,
    capturedAt: now,
    assets: [
      asset("USDC", 1, 0.01, 8_000_000_000),
      asset("EURC", 1.16, 0.05, 120_000_000),
      asset("BTC", 77_000, 3.07, 45_000_000_000),
      asset("ETH", 2_100, 4.29, 23_000_000_000),
    ],
  };
}

function asset(
  symbol: string,
  priceUsd: number,
  change24h: number,
  volume24h: number,
): MarketSnapshot["assets"][number] {
  return {
    symbol: symbol as MarketSnapshot["assets"][number]["symbol"],
    priceUsd,
    change24h,
    change7d: change24h,
    marketCap: 0,
    volume24h,
    updatedAt: new Date().toISOString(),
  };
}
