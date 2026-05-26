import { describe, expect, it } from "vitest";

import {
  chainBalanceRows,
  idleUsdcConsolidation,
  walletRouteFromBlockchain,
  walletRouteKeysFromNetworks,
  walletRouteLabel,
} from "./wallet-routes";

describe("wallet route helpers", () => {
  it("maps every supported Circle testnet route to a UI route", () => {
    expect(walletRouteFromBlockchain("ARC-TESTNET")?.key).toBe("arc");
    expect(walletRouteFromBlockchain("BASE-SEPOLIA")?.key).toBe("base");
    expect(walletRouteFromBlockchain("ETH-SEPOLIA")?.key).toBe("eth-sepolia");
    expect(walletRouteFromBlockchain("ARB-SEPOLIA")?.key).toBe("arb-sepolia");
    expect(walletRouteFromBlockchain("AVAX-FUJI")?.key).toBe("avax-fuji");
  });

  it("returns stable readable labels for tax, wallet, and dashboard surfaces", () => {
    expect(walletRouteLabel("ARC-TESTNET")).toBe("Arc testnet");
    expect(walletRouteLabel("BASE-SEPOLIA")).toBe("Base Sepolia");
    expect(walletRouteLabel("ETH-SEPOLIA")).toBe("Ethereum Sepolia");
    expect(walletRouteLabel("ARB-SEPOLIA")).toBe("Arbitrum Sepolia");
    expect(walletRouteLabel("AVAX-FUJI")).toBe("Avalanche Fuji");
  });

  it("keeps all live wallet networks visible even when balances are zero", () => {
    const routeKeys = walletRouteKeysFromNetworks([
      { blockchain: "ARC-TESTNET" },
      { blockchain: "BASE-SEPOLIA" },
      { blockchain: "ETH-SEPOLIA" },
      { blockchain: "ARB-SEPOLIA" },
      { blockchain: "AVAX-FUJI" },
    ]);

    expect(routeKeys).toEqual([
      "arc",
      "base",
      "eth-sepolia",
      "arb-sepolia",
      "avax-fuji",
    ]);
    expect(
      chainBalanceRows({
        perChainUsdc: { arc: 10, "ETH-SEPOLIA": 3 },
        perChainEurc: { "avax-fuji": 2 },
        eurcUsd: 1.1,
        routeKeys,
      }).map((row) => [row.key, row.totalUsd]),
    ).toEqual([
      ["arc", 10],
      ["base", 0],
      ["eth-sepolia", 3],
      ["arb-sepolia", 0],
      ["avax-fuji", 2.2],
    ]);
  });
});

describe("idleUsdcConsolidation (mirrors the backend routing engine)", () => {
  it("counts every non-primary chain above the bridge minimum as a source", () => {
    // Same shape as the Rust `engine_plan_consolidates_fragmented_idle_usdc_to_primary`
    // test: Arc is the primary (most idle); Base/Eth/Avax consolidate; the $1 on
    // Arbitrum is below the minimum and stays put.
    const result = idleUsdcConsolidation({
      arc: 100,
      base: 20,
      "eth-sepolia": 6,
      "avax-fuji": 5.42,
      "arb-sepolia": 1,
    });
    expect(result.sources).toBe(3);
    expect(result.fundedChains).toBe(4); // includes Arc (the primary)
  });

  it("shows no consolidation when the reserve already sits on one chain", () => {
    expect(idleUsdcConsolidation({ arc: 50, base: 1 })).toEqual({
      sources: 0,
      fundedChains: 1,
    });
  });

  it("consolidates a single non-primary chain (the case the 2+ heuristic missed)", () => {
    // Only Ethereum Sepolia holds cash; primary defaults to Base, so the backend
    // bridges eth → base and the card must appear.
    expect(idleUsdcConsolidation({ "eth-sepolia": 10 })).toEqual({
      sources: 1,
      fundedChains: 1,
    });
  });

  it("normalizes blockchain aliases and ignores unknown chains / dust", () => {
    expect(
      idleUsdcConsolidation({
        "ARC-TESTNET": 100,
        "BASE-SEPOLIA": 9,
        solana: 50, // unknown route → not a CCTP execution chain here
        "arb-sepolia": 0.4, // dust below the minimum
      }),
    ).toEqual({ sources: 1, fundedChains: 2 }); // Arc primary, Base is the source
  });
});
