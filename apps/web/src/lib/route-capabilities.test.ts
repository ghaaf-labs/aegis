import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { allocationDisplayMeta } from "./route-capabilities";

describe("allocationDisplayMeta", () => {
  it("maps backend execution symbols to friendly display labels", () => {
    expect(allocationDisplayMeta("cbBTC").label).toBe("Bitcoin");
    expect(allocationDisplayMeta("ETH").label).toBe("Ethereum");
    expect(allocationDisplayMeta("EURC").label).toBe("Euro Coin");
    expect(allocationDisplayMeta("USDC").label).toBe("Cash (USDC)");
    expect(allocationDisplayMeta("SOL").label).toBe("Solana");
  });

  it("falls back to the raw symbol for unknown tokens (never crashes)", () => {
    expect(allocationDisplayMeta("XYZ").label).toBe("XYZ");
  });

  it("assigns honest per-asset route states", () => {
    // Cash executes immediately.
    expect(allocationDisplayMeta("USDC").routeState).toBe("executes-now");
    // USYC is gated → coming-soon, never an investable "ready" target.
    expect(allocationDisplayMeta("USYC").routeState).toBe("coming-soon");
    // A designable sleeve whose rail is offline is an honest pending target —
    // never silently relabelled as USDC or dropped.
    expect(allocationDisplayMeta("cbBTC").routeState).toBe(
      "target-pending-rail",
    );
    expect(allocationDisplayMeta("SOL").routeState).toBe("target-pending-rail");
    expect(allocationDisplayMeta("EURC").routeState).toBe(
      "target-pending-rail",
    );
  });

  it("gives each route state a human badge", () => {
    expect(allocationDisplayMeta("USDC").badge).toBe("Executes now");
    expect(allocationDisplayMeta("USYC").badge).toBe("Coming soon");
    expect(allocationDisplayMeta("cbBTC").badge).toBe(
      "Executes when rail live",
    );
  });
});

describe("isTradeableSleeve", () => {
  // The flag is read at module-load, so each case sets the env then imports a
  // fresh module — never relying on the ambient value at file-evaluation time.
  const original = process.env.NEXT_PUBLIC_VOLATILE_EXECUTION_ENABLED;
  beforeEach(() => {
    delete process.env.NEXT_PUBLIC_VOLATILE_EXECUTION_ENABLED;
    vi.resetModules();
  });
  afterEach(() => {
    if (original === undefined) {
      delete process.env.NEXT_PUBLIC_VOLATILE_EXECUTION_ENABLED;
    } else {
      process.env.NEXT_PUBLIC_VOLATILE_EXECUTION_ENABLED = original;
    }
    vi.resetModules();
  });

  it("treats stablecoins as tradeable and non-stables as tracked while volatile execution is off", async () => {
    // Default deployment (testnet): only the stablecoin layer rebalances.
    const { isTradeableSleeve } = await import("./route-capabilities");
    expect(isTradeableSleeve("USDC")).toBe(true);
    expect(isTradeableSleeve("ETH")).toBe(false);
    expect(isTradeableSleeve("cbBTC")).toBe(false);
    expect(isTradeableSleeve("EURC")).toBe(false);
    // Coming-soon / gated sleeves are never tradeable.
    expect(isTradeableSleeve("USYC")).toBe(false);
    // Unknown symbols never crash and are not tradeable.
    expect(isTradeableSleeve("XYZ")).toBe(false);
  });

  it("flips only volatile sleeves to tradeable when VOLATILE_EXECUTION_ENABLED is on (mainnet)", async () => {
    process.env.NEXT_PUBLIC_VOLATILE_EXECUTION_ENABLED = "true";
    vi.resetModules();
    const { isTradeableSleeve } = await import("./route-capabilities");

    expect(isTradeableSleeve("USDC")).toBe(true);
    expect(isTradeableSleeve("ETH")).toBe(true);
    expect(isTradeableSleeve("cbBTC")).toBe(true);
    // FX/yield sleeves are gated by their own rails (StableFX KYB, USYC_ENABLED),
    // not the volatile flag — they stay tracked in this coarse predicate.
    expect(isTradeableSleeve("EURC")).toBe(false);
    expect(isTradeableSleeve("USYC")).toBe(false);
  });
});
