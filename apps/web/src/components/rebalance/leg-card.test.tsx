import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { LegCard } from "./leg-card";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
});

function renderLegCard(props: Parameters<typeof LegCard>[0]) {
  const container = document.createElement("div");
  document.body.appendChild(container);
  let root: Root;
  act(() => {
    root = createRoot(container);
    root.render(<LegCard {...props} />);
  });
  const link = container.querySelector("a") as HTMLAnchorElement | null;
  return link?.getAttribute("href") ?? null;
}

describe("<LegCard /> explorer link", () => {
  it("links a CCTP burn to the source-chain explorer (where the burn runs)", () => {
    const href = renderLegCard({
      legIndex: 0,
      kind: "cross_chain_burn",
      srcChain: "base",
      destChain: "arc",
      srcSymbol: "USDC",
      destSymbol: "USDC",
      amountUsdc: 1000,
      status: "confirmed",
      txHash: "0xburn",
    });
    expect(href).toContain("basescan.org");
    expect(href).toContain("0xburn");
  });

  it("links a CCTP mint to the destination-chain explorer", () => {
    const href = renderLegCard({
      legIndex: 1,
      kind: "cross_chain_mint",
      srcChain: "base",
      destChain: "arc",
      srcSymbol: "USDC",
      destSymbol: "USDC",
      amountUsdc: 1000,
      status: "confirmed",
      txHash: "0xmint",
    });
    expect(href).toContain("arcscan.app");
    expect(href).toContain("0xmint");
  });
});
