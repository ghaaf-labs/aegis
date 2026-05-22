import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import OnboardingPage from "./page";
import { walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

const replace = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    replace,
  }),
}));

vi.mock("@/components/onboarding/goal-wizard", () => ({
  GoalWizard: () => <div data-testid="goal-wizard">Goal wizard</div>,
}));

vi.mock("@/lib/api", () => ({
  walletApi: {
    session: vi.fn(),
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
  usePortfolioStore.getState().resetSession();
  window.localStorage.clear();
});

describe("<OnboardingPage /> auth boundary", () => {
  it("shows the verified session and logout before rendering portfolio setup", async () => {
    vi.mocked(walletApi.session).mockResolvedValue({
      user: {
        id: "user-1",
        email: "user@example.com",
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
    vi.mocked(walletApi.logout).mockResolvedValue(undefined);

    const { container, root } = render(<OnboardingPage />);
    await flushEffects();

    expect(container.textContent).toContain("Create your portfolio");
    expect(container.textContent).toContain("user@example.com");
    expect(usePortfolioStore.getState().sessionActive).toBe(true);
    expect(usePortfolioStore.getState().sessionResolved).toBe(true);
    expect(
      container.querySelector('[data-testid="goal-wizard"]'),
    ).not.toBeNull();

    const logout = buttonByText(container, "Log out");
    await act(async () => {
      logout.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(walletApi.logout).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith("/login?signedOut=1");

    act(() => root.unmount());
  });

  it("blocks portfolio setup while the account is still finishing", async () => {
    vi.mocked(walletApi.session).mockResolvedValueOnce({
      user: {
        id: "user-1",
        email: "pending@example.com",
        riskTolerance: "moderate",
        accountStatus: "pending_wallet",
      },
      accountStatus: "pending_wallet",
      wallet: null,
    });
    vi.mocked(walletApi.session).mockResolvedValueOnce({
      user: {
        id: "user-1",
        email: "pending@example.com",
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

    const { container, root } = render(<OnboardingPage />);
    await flushEffects();

    expect(container.textContent).toContain("Setting up your account");
    expect(container.textContent).toContain("pending@example.com");
    expect(usePortfolioStore.getState().sessionActive).toBe(true);
    expect(usePortfolioStore.getState().sessionResolved).toBe(true);
    expect(usePortfolioStore.getState().wallet).toBe(null);
    expect(container.textContent).not.toContain("Circle");
    expect(container.textContent).not.toContain("Arc + Base");
    expect(container.querySelector('[data-testid="goal-wizard"]')).toBeNull();
    expect(container.textContent).not.toContain("Try again");

    const retry = buttonByText(container, "Check again");
    await act(async () => {
      retry.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(walletApi.session).toHaveBeenCalledTimes(2);
    expect(
      container.querySelector('[data-testid="goal-wizard"]'),
    ).not.toBeNull();

    act(() => root.unmount());
  });

  it("keeps pending-account retry errors user-safe", async () => {
    vi.mocked(walletApi.session).mockResolvedValueOnce({
      user: {
        id: "user-1",
        email: "pending@example.com",
        riskTolerance: "moderate",
        accountStatus: "pending_wallet",
      },
      accountStatus: "pending_wallet",
      wallet: null,
    });
    vi.mocked(walletApi.session).mockRejectedValueOnce(new Error("offline"));

    const { container, root } = render(<OnboardingPage />);
    await flushEffects();

    const retry = buttonByText(container, "Check again");
    await act(async () => {
      retry.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(container.textContent).toContain(
      "We could not finish account setup. Try again.",
    );
    expect(container.textContent).not.toContain("Aegis could not");
    expect(container.textContent).not.toContain("backend");
    expect(container.textContent).not.toContain("session");

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

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function buttonByText(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find(
    (candidate) => candidate.textContent?.includes(text),
  );
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error(`Missing button: ${text}`);
  }
  return button;
}
