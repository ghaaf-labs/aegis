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
    me: vi.fn(),
    status: vi.fn(),
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
    vi.mocked(walletApi.me).mockResolvedValue({
      id: "user-1",
      email: "user@example.com",
      riskTolerance: "moderate",
    });
    vi.mocked(walletApi.status).mockResolvedValue({
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

    expect(container.textContent).toContain("SESSION VERIFIED");
    expect(container.textContent).toContain("user@example.com");
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

  it("blocks portfolio setup when the session has no real wallet yet", async () => {
    vi.mocked(walletApi.me).mockResolvedValue({
      id: "user-1",
      email: "pending@example.com",
      riskTolerance: "moderate",
    });
    vi.mocked(walletApi.status).mockResolvedValue({ wallet: null });

    const { container, root } = render(<OnboardingPage />);
    await flushEffects();

    expect(container.textContent).toContain("Finish wallet setup first");
    expect(container.textContent).toContain("pending@example.com");
    expect(container.querySelector('[data-testid="goal-wizard"]')).toBeNull();

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
