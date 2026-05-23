import { describe, expect, it } from "vitest";
import { deriveCashSplit, totalWalletCashUsd } from "./cash-model";
import { chainBalanceRows } from "./wallet-routes";
import type { MarketSnapshot } from "@/types";

const eurcSnapshot = {
  id: "s1",
  capturedAt: new Date().toISOString(),
  fearGreedIndex: 50,
  totalMarketCapUsd: 0,
  btcDominance: 0,
  assets: [
    {
      symbol: "EURC",
      priceUsd: 1.16,
      change24h: 0,
      change7d: 0,
      marketCap: 0,
      volume24h: 0,
      updatedAt: new Date().toISOString(),
    },
  ],
} satisfies MarketSnapshot;

describe("totalWalletCashUsd", () => {
  it("marks EURC wallet cash to USD before adding it to USDC", () => {
    expect(totalWalletCashUsd(25, 100, eurcSnapshot)).toBe(141);
  });

  it("falls back to the stable EURC rate when the snapshot lacks a mark", () => {
    // 1.085 fallback: 100 EURC ≈ 108.5.
    expect(totalWalletCashUsd(0, 100, null)).toBeCloseTo(108.5, 6);
  });
});

describe("deriveCashSplit", () => {
  it("reserves the USDC target weight and only deploys the surplus", () => {
    // Target 80% USDC; $100 invested + $32 idle USDC ⇒ $132 plan basis ⇒
    // reserve = 0.8 * 132 = 105.6, which exceeds idle, so surplus is $0.
    const split = deriveCashSplit({
      unifiedUsdc: 32,
      unifiedEurc: 0,
      targetAllocations: [{ symbol: "USDC", targetWeight: 80 }],
      investedUsd: 100,
      snapshot: null,
    });
    expect(split.reserveUsd).toBeCloseTo(105.6, 6);
    expect(split.deployableUsd).toBe(0);
    expect(split.hasUsdcReserveTarget).toBe(true);
  });

  it("deploys idle surplus above a small reserve", () => {
    // 10% USDC reserve on a $200 basis = $20 reserve; $50 idle ⇒ $30 deployable.
    const split = deriveCashSplit({
      unifiedUsdc: 50,
      unifiedEurc: 0,
      targetAllocations: [{ symbol: "USDC", targetWeight: 10 }],
      investedUsd: 150,
      snapshot: null,
    });
    expect(split.reserveUsd).toBeCloseTo(20, 6);
    expect(split.deployableUsd).toBeCloseTo(30, 6);
  });

  it("treats all idle USDC as deployable when no USDC reserve target exists", () => {
    const split = deriveCashSplit({
      unifiedUsdc: 40,
      unifiedEurc: 0,
      targetAllocations: [{ symbol: "BTC", targetWeight: 100 }],
      investedUsd: 0,
      snapshot: null,
    });
    expect(split.hasUsdcReserveTarget).toBe(false);
    expect(split.reserveUsd).toBe(0);
    expect(split.deployableUsd).toBe(40);
  });
});

describe("per-chain ↔ unified reconciliation invariant", () => {
  it("sum of every chain row equals the unified wallet headline", () => {
    // The headline equals the sum of per-chain balances Circle reports; every
    // chain with a balance must appear in a row so the two reconcile.
    const perChainUsdc = { arc: 6, base: 6, "eth-sepolia": 20 };
    const perChainEurc = { base: 10 };
    const eurcUsd = 1.1;
    const unifiedUsdc = Object.values(perChainUsdc).reduce((a, b) => a + b, 0);
    const unifiedEurc = Object.values(perChainEurc).reduce((a, b) => a + b, 0);
    const headline = unifiedUsdc + unifiedEurc * eurcUsd;

    const rows = chainBalanceRows({ perChainUsdc, perChainEurc, eurcUsd });
    const rowSum = rows.reduce((sum, row) => sum + row.totalUsd, 0);

    expect(rowSum).toBeCloseTo(headline, 6);
    // The Ethereum balance is not silently dropped.
    expect(rows.some((r) => r.key === "eth-sepolia" && r.usdc === 20)).toBe(
      true,
    );
  });
});
