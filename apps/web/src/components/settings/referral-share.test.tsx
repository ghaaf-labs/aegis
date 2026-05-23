import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

import { ReferralShare } from "./referral-share";
import { handleForUserId } from "@/lib/md5";

const session = vi.fn();
const listReferrals = vi.fn();
const copyTextToClipboard = vi.fn();

vi.mock("@/lib/api", () => ({
  walletApi: { session: (...a: unknown[]) => session(...a) },
  billingApi: { listReferrals: (...a: unknown[]) => listReferrals(...a) },
}));

vi.mock("@/lib/clipboard", () => ({
  copyTextToClipboard: (...a: unknown[]) => copyTextToClipboard(...a),
}));

const USER_ID = "11111111-1111-1111-1111-111111111111";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
});

describe("<ReferralShare />", () => {
  it("builds the share link from the user's md5 handle and copies it", async () => {
    session.mockResolvedValue({ user: { id: USER_ID, email: "a@b.co" } });
    listReferrals.mockResolvedValue({
      rows: [],
      totalPaidUsdc: 0,
      totalPendingUsdc: 0,
    });
    copyTextToClipboard.mockResolvedValue(undefined);

    const { container, root } = render(<ReferralShare />);
    await flushEffects();

    const handle = handleForUserId(USER_ID);
    const code = container.querySelector("code");
    expect(code?.textContent).toBe(
      `${window.location.origin}/signup?ref=${handle}`,
    );
    expect(container.textContent).toContain(
      "No referrals yet. Share your link to start earning.",
    );

    const copyBtn = button(container, "Copy link");
    act(() => copyBtn.click());
    await flushEffects();

    expect(copyTextToClipboard).toHaveBeenCalledWith(
      `${window.location.origin}/signup?ref=${handle}`,
    );
    expect(container.textContent).toContain("Copied");

    act(() => root.unmount());
  });

  it("renders real referral rows and payout totals", async () => {
    session.mockResolvedValue({ user: { id: USER_ID, email: "a@b.co" } });
    listReferrals.mockResolvedValue({
      rows: [
        {
          id: "ref-1",
          newUserId: "u2",
          rewardUsdc: 5,
          paidAt: "2026-05-01T00:00:00Z",
          txHash: "0xabc",
          createdAt: "2026-05-01T00:00:00Z",
        },
        {
          id: "ref-2",
          newUserId: "u3",
          rewardUsdc: 5,
          paidAt: null,
          txHash: null,
          createdAt: "2026-05-10T00:00:00Z",
        },
      ],
      totalPaidUsdc: 5,
      totalPendingUsdc: 5,
    });

    const { container, root } = render(<ReferralShare />);
    await flushEffects();

    expect(container.textContent).toContain("$5.00 USDC");
    expect(container.textContent).toContain("paid");
    expect(container.textContent).toContain("pending");
    expect(container.textContent).not.toContain("No referrals yet");

    act(() => root.unmount());
  });

  it("shows a sign-in prompt when the session has no user id", async () => {
    session.mockRejectedValue(new Error("401"));
    listReferrals.mockRejectedValue(new Error("401"));

    const { container, root } = render(<ReferralShare />);
    await flushEffects();

    expect(container.textContent).toContain(
      "Sign in to load your referral link.",
    );
    expect(container.querySelector("code")).toBeNull();

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

function button(container: HTMLElement, label: string): HTMLButtonElement {
  const found = Array.from(container.querySelectorAll("button")).find((b) =>
    b.textContent?.includes(label),
  );
  if (!found) throw new Error(`missing button: ${label}`);
  return found as HTMLButtonElement;
}

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
