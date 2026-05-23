import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { AppShell } from "./app-shell";
import { usePortfolioStore } from "@/stores/portfolio";

let mockPathname = "/agent-studio";

vi.mock("next/navigation", () => ({
  usePathname: () => mockPathname,
  useRouter: () => ({
    push: vi.fn(),
  }),
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
  gatewayApi: {
    balance: vi.fn(),
  },
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
  mockPathname = "/agent-studio";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
  window.localStorage.clear();
});

describe("<AppShell />", () => {
  it("keeps the app frame available while a protected route is checking auth", async () => {
    const { root, container } = render(
      <AppShell>
        <div>Agent Studio content</div>
      </AppShell>,
    );
    await flushEffects();

    expect(container.textContent).not.toContain("Checking your session");
    expect(container.textContent).toContain("Dashboard overview");
    expect(container.textContent).toContain("Agent Studio content");

    act(() => root.unmount());
  });

  it("shows one clear login action for signed-out protected routes", async () => {
    act(() => {
      usePortfolioStore.getState().setSessionResolved(true);
      usePortfolioStore.getState().setSessionActive(false);
    });

    const { root, container } = render(
      <AppShell>
        <div>Agent Studio content</div>
      </AppShell>,
    );
    await flushEffects();

    const link = container.querySelector<HTMLAnchorElement>("a");
    expect(container.textContent).toContain("Continue with email");
    expect(link?.getAttribute("href")).toBe(
      "/login?next=%2Fagent-studio&reason=session_required",
    );
    expect(container.textContent).not.toContain("Agent Studio content");

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
