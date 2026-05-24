import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Sidebar } from "./sidebar";
import { usePortfolioStore } from "@/stores/portfolio";

let mockPathname = "/help";

vi.mock("next/navigation", () => ({
  usePathname: () => mockPathname,
}));

vi.mock("next/link", () => ({
  default: ({
    href,
    children,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement> & {
    href: string;
    children: React.ReactNode;
  }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("@/lib/api", () => ({
  userAgentApi: {
    status: vi.fn().mockResolvedValue({ pausedAt: null }),
  },
  walletApi: {
    logout: vi.fn().mockResolvedValue(undefined),
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
  mockPathname = "/help";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
  window.localStorage.clear();
});

describe("<Sidebar />", () => {
  it("hides protected destinations from the DOM when session is confirmed absent", async () => {
    act(() => {
      usePortfolioStore.getState().setSessionResolved(true);
      usePortfolioStore.getState().setSessionActive(false);
    });

    const { root, container } = render(<Sidebar />);
    await flushEffects();

    // Protected links must not be in the DOM at all for signed-out users.
    const dashboardLink = container.querySelector<HTMLAnchorElement>(
      'a[href="/dashboard"]',
    );
    expect(dashboardLink).toBeNull();

    // Public destinations must be present.
    const exploreLink =
      container.querySelector<HTMLAnchorElement>('a[href="/explore"]');
    expect(exploreLink).not.toBeNull();
    expect(container.textContent).toContain("Explore demos");
    expect(container.textContent).toContain("Leaderboard");
    expect(container.textContent).toContain("Regime model");
    expect(container.textContent).toContain("Help");

    act(() => root.unmount());
  });

  it("shows full nav rail with auth-aware locked state while session is resolving", async () => {
    act(() => {
      usePortfolioStore.getState().setSessionResolved(true);
      usePortfolioStore.getState().setSessionActive(true);
      usePortfolioStore.getState().setWallet(null);
    });

    const { root, container } = render(<Sidebar />);
    await flushEffects();

    // Authenticated but wallet-pending: full rail visible, wallet-recovery items unlocked.
    const dashboardLink = container.querySelector<HTMLAnchorElement>(
      'a[aria-label="Dashboard: account setup pending"]',
    );
    expect(dashboardLink).not.toBeNull();
    expect(
      container.querySelector('[data-testid="sidebar-logout"]'),
    ).not.toBeNull();
    expect(container.textContent).toContain("Sign out");
    expect(container.textContent).not.toContain("Sign out from the top bar.");

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
