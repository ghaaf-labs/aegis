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
  it("separates nav labels from route state for assistive tech", async () => {
    act(() => {
      usePortfolioStore.getState().setSessionResolved(true);
    });

    const { root, container } = render(<Sidebar />);
    await flushEffects();

    const dashboardLink = container.querySelector<HTMLAnchorElement>(
      'a[aria-label="Dashboard: sign in required"]',
    );
    expect(dashboardLink?.getAttribute("aria-label")).toBe(
      "Dashboard: sign in required",
    );
    expect(dashboardLink?.textContent).toContain("Dashboard sign in required");

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
