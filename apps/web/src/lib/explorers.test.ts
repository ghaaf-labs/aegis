import { describe, expect, it } from "vitest";

import { explorerAddressUrl, explorerTxUrl } from "./explorers";

describe("explorer URL helpers", () => {
  it("builds current Arc testnet transaction and address links", () => {
    expect(explorerTxUrl("arc", "0xabc")).toBe(
      "https://testnet.arcscan.app/tx/0xabc",
    );
    expect(explorerAddressUrl("arc", "0x123")).toBe(
      "https://testnet.arcscan.app/address/0x123",
    );
  });

  it("builds Base Sepolia transaction and address links", () => {
    expect(explorerTxUrl("base", "0xabc")).toBe(
      "https://sepolia.basescan.org/tx/0xabc",
    );
    expect(explorerAddressUrl("base", "0x123")).toBe(
      "https://sepolia.basescan.org/address/0x123",
    );
  });

  it("does not produce broken links for missing inputs", () => {
    expect(explorerTxUrl(null, "0xabc")).toBeNull();
    expect(explorerTxUrl("arc", null)).toBeNull();
    expect(explorerAddressUrl(undefined, "0x123")).toBeNull();
    expect(explorerAddressUrl("base", undefined)).toBeNull();
  });
});
