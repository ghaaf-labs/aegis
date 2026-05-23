import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { EmailAuthCard } from "./email-auth-card";
import { walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";
import type { Portfolio } from "@/types";

const routerReplace = vi.fn();
let mockSearchParams = new URLSearchParams();

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: routerReplace,
  }),
  useSearchParams: () => mockSearchParams,
}));

vi.mock("@/lib/api", () => ({
  analyticsApi: {
    track: vi.fn().mockResolvedValue(undefined),
  },
  walletApi: {
    session: vi.fn(),
    startEmail: vi.fn(),
    resendEmail: vi.fn(),
    verifyEmail: vi.fn(),
    logout: vi.fn(),
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
  mockSearchParams = new URLSearchParams();
  usePortfolioStore.getState().resetSession();
  window.localStorage.clear();
});

describe("<EmailAuthCard />", () => {
  it("keeps the email form closed while the account check is pending", async () => {
    vi.mocked(walletApi.session).mockReturnValue(new Promise(() => {}));

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    expect(container.textContent).toContain("Checking sign-in");
    expect(container.textContent).toContain("One moment.");
    expect(container.querySelector('[data-testid="wallet-auth-email"]')).toBe(
      null,
    );
    expect(walletApi.startEmail).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("shows one minimal email form when no account is open in this browser", async () => {
    window.localStorage.setItem("aegis_email", "remembered@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    const emailInput = container.querySelector<HTMLInputElement>(
      '[data-testid="wallet-auth-email"]',
    );
    expect(emailInput?.value).toBe("");
    expect(container.textContent).toContain("Continue with email");
    expect(container.textContent).toContain("We'll email you a 6-digit code.");

    act(() => root.unmount());
  });

  it("explains why an invalid email cannot continue", async () => {
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await fill(container, '[data-testid="wallet-auth-email"]', "bad-email");
    await flushEffects();

    const submit = container.querySelector<HTMLButtonElement>(
      '[data-testid="wallet-auth-submit"]',
    );
    const input = container.querySelector<HTMLInputElement>(
      '[data-testid="wallet-auth-email"]',
    );
    expect(container.textContent).toContain("Enter a valid email address.");
    expect(input?.getAttribute("aria-invalid")).toBe("true");
    expect(input?.getAttribute("aria-describedby")).toContain(
      "wallet-auth-email-invalid",
    );
    expect(submit?.disabled).toBe(true);

    act(() => root.unmount());
  });

  it("redirects to the app when this browser is already in Aegis", async () => {
    vi.mocked(walletApi.session).mockResolvedValue({
      user: {
        id: "user-1",
        email: "verified@example.com",
        riskTolerance: "moderate",
        accountStatus: "active",
      },
      accountStatus: "active",
      wallet: {
        walletId: "wallet-live",
        arcAddress: "0x1111111111111111111111111111111111111111",
        baseAddress: "0x2222222222222222222222222222222222222222",
        createdAt: new Date().toISOString(),
      },
    });

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await flushEffects();

    expect(routerReplace).toHaveBeenCalledWith("/dashboard");
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );
    expect(walletApi.startEmail).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("starts auth without a signup/login branch", async () => {
    mockSearchParams = new URLSearchParams("email=exists@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-1",
      email: "exists@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
      devCode: "123456",
    });

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    expect(walletApi.startEmail).toHaveBeenCalledTimes(1);
    expect(walletApi.startEmail).toHaveBeenCalledWith(
      "exists@example.com",
      undefined,
    );
    expect(container.textContent).toContain("Enter the code we emailed you");
    expect(container.textContent).toContain("Sent to");
    expect(container.querySelector(".animate-spin")).toBe(null);
    expect(container.textContent).not.toContain("This email is new");
    expect(container.textContent).not.toContain("Mock dev code");
    expect(container.textContent).not.toContain("Local code");
    expect(container.textContent).not.toContain("123456");

    act(() => root.unmount());
  });

  it("opens the app after a verified account is ready", async () => {
    mockSearchParams = new URLSearchParams("email=new@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-2",
      email: "new@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
    });
    vi.mocked(walletApi.verifyEmail).mockResolvedValue({
      status: "active",
      user: {
        id: "user-2",
        email: "new@example.com",
        riskTolerance: "moderate",
        accountStatus: "active",
      },
      wallet: {
        walletId: "wallet-live",
        arcAddress: "0x1111111111111111111111111111111111111111",
        baseAddress: "0x2222222222222222222222222222222222222222",
        createdAt: new Date().toISOString(),
      },
    });

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();
    await fill(container, '[data-testid="wallet-auth-code"]', "123456");
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    expect(walletApi.verifyEmail).toHaveBeenCalledWith("code-2", "123456", {
      tos: true,
      privacy: true,
      tosVersion: "2026-05",
      privacyVersion: "2026-05",
      marketingOptIn: false,
    });
    expect(routerReplace).toHaveBeenCalledWith("/dashboard");

    act(() => root.unmount());
  });

  it("marks the code field invalid when the server rejects a code", async () => {
    mockSearchParams = new URLSearchParams("email=wrong-code@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-wrong",
      email: "wrong-code@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
    });
    vi.mocked(walletApi.verifyEmail).mockRejectedValue(
      new Error("400: code_invalid"),
    );

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();
    await fill(container, '[data-testid="wallet-auth-code"]', "000000");
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    const codeInput = container.querySelector<HTMLInputElement>(
      '[data-testid="wallet-auth-code"]',
    );
    expect(codeInput?.getAttribute("aria-invalid")).toBe("true");
    expect(codeInput?.getAttribute("aria-describedby")).toBe(
      "wallet-auth-error",
    );
    expect(container.textContent).toContain(
      "That code did not match. Check it or request a new one.",
    );

    act(() => root.unmount());
  });

  it("clears stale portfolio state before opening a verified account", async () => {
    mockSearchParams = new URLSearchParams("email=new-owner@example.com");
    act(() => {
      usePortfolioStore.getState().setPortfolios([stalePortfolio()]);
    });
    expect(usePortfolioStore.getState().portfolios).toHaveLength(1);

    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-clean",
      email: "new-owner@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
    });
    vi.mocked(walletApi.verifyEmail).mockResolvedValue({
      status: "active",
      user: {
        id: "user-clean",
        email: "new-owner@example.com",
        riskTolerance: "moderate",
        accountStatus: "active",
      },
      wallet: {
        walletId: "wallet-clean",
        arcAddress: "0x1111111111111111111111111111111111111111",
        baseAddress: "0x2222222222222222222222222222222222222222",
        createdAt: new Date().toISOString(),
      },
    });

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();
    await fill(container, '[data-testid="wallet-auth-code"]', "123456");
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    expect(routerReplace).toHaveBeenCalledWith("/dashboard");
    expect(usePortfolioStore.getState().portfolios).toHaveLength(0);
    expect(usePortfolioStore.getState().activePortfolioId).toBe(null);
    expect(usePortfolioStore.getState().sessionActive).toBe(true);
    expect(usePortfolioStore.getState().wallet?.walletId).toBe("wallet-clean");

    act(() => root.unmount());
  });

  it("uses a calm finishing state when the account is not ready yet", async () => {
    mockSearchParams = new URLSearchParams("email=slow@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-3",
      email: "slow@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
    });
    vi.mocked(walletApi.verifyEmail).mockResolvedValue({
      status: "provisioning",
      user: {
        id: "user-3",
        email: "slow@example.com",
        riskTolerance: "moderate",
        accountStatus: "pending_wallet",
      },
      wallet: null,
    });

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();
    await fill(container, '[data-testid="wallet-auth-code"]', "123456");
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();
    await flushEffects();

    expect(container.textContent).toContain("Setting up your account");
    expect(container.textContent).toContain("This is taking longer than usual");
    expect(container.textContent).toContain("Try again");
    expect(walletApi.verifyEmail).toHaveBeenCalledTimes(1);
    expect({
      sessionActive: usePortfolioStore.getState().sessionActive,
      sessionResolved: usePortfolioStore.getState().sessionResolved,
      wallet: usePortfolioStore.getState().wallet,
      email: window.localStorage.getItem("aegis_email"),
    }).toEqual({
      sessionActive: true,
      sessionResolved: true,
      wallet: null,
      email: "slow@example.com",
    });
    expect(container.textContent).not.toContain("PIN");
    expect(container.textContent).not.toContain("Circle");

    act(() => root.unmount());
  });

  it("clears remembered email hints after a signed-out redirect", async () => {
    mockSearchParams = new URLSearchParams("signedOut=1");
    window.localStorage.setItem("aegis_email", "stale@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    expect(window.localStorage.getItem("aegis_email")).toBe(null);
    expect(container.textContent).toContain(
      "Signed out. Enter your email to continue.",
    );

    act(() => root.unmount());
  });

  it("shows a plain continuation note for a routine protected-page redirect", async () => {
    mockSearchParams = new URLSearchParams(
      "next=%2Fdashboard&reason=session_required",
    );
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    expect(container.textContent).toContain("Continue with email");
    expect(container.textContent).toContain("We'll email you a 6-digit code.");
    expect(container.textContent).toContain("Enter your email to continue.");

    act(() => root.unmount());
  });

  it("keeps failed sign-in check copy user-safe", async () => {
    mockSearchParams = new URLSearchParams("reason=session_check_failed");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    expect(container.textContent).toContain(
      "We could not verify this sign-in. Enter your email to continue.",
    );
    expect(container.textContent).not.toContain("browser");
    expect(container.textContent).not.toContain("session check");

    act(() => root.unmount());
  });

  it("resends the current challenge after the cooldown", async () => {
    mockSearchParams = new URLSearchParams("email=resend@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-4",
      email: "resend@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 0,
    });
    vi.mocked(walletApi.resendEmail).mockResolvedValue({
      challengeId: "code-4",
      email: "resend@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
    });

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    await clickByText(container, "Resend code");
    await flushEffects();

    expect(walletApi.resendEmail).toHaveBeenCalledWith("code-4");
    expect(walletApi.startEmail).toHaveBeenCalledTimes(1);

    act(() => root.unmount());
  });

  it("returns to email entry when the current code is exhausted", async () => {
    mockSearchParams = new URLSearchParams("email=locked@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockResolvedValue({
      challengeId: "code-5",
      email: "locked@example.com",
      expiresAt: new Date(Date.now() + 600_000).toISOString(),
      resendInSeconds: 30,
    });
    vi.mocked(walletApi.verifyEmail).mockRejectedValue(
      new Error("429: too_many_attempts"),
    );

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();
    await fill(container, '[data-testid="wallet-auth-code"]', "000000");
    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    expect(container.textContent).toContain("Continue with email");
    expect(container.textContent).toContain(
      "Too many tries. Enter your email to get a new code.",
    );
    expect(container.querySelector('[data-testid="wallet-auth-code"]')).toBe(
      null,
    );

    act(() => root.unmount());
  });

  it("keeps connection errors free of internal product blame", async () => {
    mockSearchParams = new URLSearchParams("email=offline@example.com");
    vi.mocked(walletApi.session).mockRejectedValue(new Error("missing"));
    vi.mocked(walletApi.startEmail).mockRejectedValue(
      new Error("NetworkError: failed to fetch"),
    );

    const { root, container } = render(<EmailAuthCard />);
    await flushEffects();

    await click(container, '[data-testid="wallet-auth-submit"]');
    await flushEffects();

    expect(container.textContent).toContain("We could not connect. Try again.");
    expect(container.textContent).not.toContain("Aegis could not");
    expect(container.textContent).not.toContain("backend");

    act(() => root.unmount());
  });
});

function stalePortfolio(): Portfolio {
  return {
    id: "old-portfolio",
    userId: "old-user",
    name: "Previous account",
    totalValueUsd: 100,
    totalPnlUsd: 0,
    totalPnlPct: 0,
    allocations: [],
    riskScore: 0,
    goal: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
}

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

async function click(container: HTMLElement, selector: string) {
  const element = container.querySelector<HTMLElement>(selector);
  expect(element).not.toBe(null);
  await act(async () => {
    element!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
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

async function fill(container: HTMLElement, selector: string, value: string) {
  const input = container.querySelector<HTMLInputElement>(selector);
  expect(input).not.toBe(null);
  await act(async () => {
    const setter = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set;
    setter?.call(input, value);
    input!.dispatchEvent(
      new InputEvent("input", {
        bubbles: true,
        inputType: "insertText",
        data: value,
      }),
    );
  });
}

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
