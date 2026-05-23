import { describe, expect, it } from "vitest";

import {
  chainBalanceRows,
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
