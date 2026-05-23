import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

import { FaucetButton } from "./faucet-button";
import { faucetApi } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  faucetApi: {
    claim: vi.fn(),
  },
}));

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  delete (navigator as unknown as Record<string, unknown>).clipboard;
  delete (document as unknown as Record<string, unknown>).execCommand;
});

describe("<FaucetButton />", () => {
  it("shows a manual address when clipboard writes are blocked", async () => {
    vi.spyOn(window, "open").mockImplementation(() => null);
    vi.mocked(faucetApi.claim).mockResolvedValue({
      amountUsdc: 100,
      chain: "arc-testnet",
      txHash: null,
      remainingTodayUsdc: 0,
      claimedAt: new Date().toISOString(),
      claimUrl: "https://faucet.circle.com",
      arcAddress: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
    });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockRejectedValue(new DOMException("blocked")),
      },
    });
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: vi.fn().mockReturnValue(false),
    });

    const { container, root } = render(<FaucetButton />);
    await clickByText(container, "Get test USDC");
    await flushEffects();
    await clickByText(container, "0x8955c4");
    await flushEffects();

    expect(container.textContent).toContain("Copy failed");
    expect(container.textContent).toContain(
      "Use this address: 0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
    );

    act(() => root.unmount());
  });

  it("keeps faucet limit errors actionable and free of HTTP prefixes", async () => {
    vi.mocked(faucetApi.claim).mockRejectedValue(
      new Error(
        "429: You already requested today's test USDC. Open the faucet directly or try again tomorrow.",
      ),
    );

    const { container, root } = render(<FaucetButton />);
    await clickByText(container, "Get test USDC");
    await flushEffects();

    expect(container.textContent).toContain(
      "Daily test funds already requested",
    );
    expect(container.textContent).toContain("try again tomorrow");
    expect(container.textContent).not.toContain("429:");
    expect(container.querySelector("button")).toBeNull();
    expect(
      container.querySelector<HTMLAnchorElement>(
        'a[href="https://faucet.circle.com"]',
      )?.textContent,
    ).toContain("Open faucet");

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

async function clickByText(container: HTMLElement, text: string) {
  const element = [...container.querySelectorAll<HTMLElement>("button")].find(
    (button) => button.textContent?.includes(text),
  );
  expect(element).not.toBe(null);
  await act(async () => {
    element!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
}

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
