import { describe, expect, it } from "vitest";
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
