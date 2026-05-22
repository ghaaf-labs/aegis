import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { AuthGate } from "./auth-gate";
import { usePortfolioStore } from "@/stores/portfolio";

const routerReplace = vi.fn();
let mockPathname = "/wallets";

vi.mock("next/navigation", () => ({
  usePathname: () => mockPathname,
  useRouter: () => ({
    replace: routerReplace,
  }),
}));

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
  routerReplace.mockClear();
  mockPathname = "/wallets";
  usePortfolioStore.getState().resetSession();
  usePortfolioStore.getState().setSessionResolved(false);
  window.localStorage.clear();
});

describe("<AuthGate />", () => {
  it("hides protected content immediately when logout clears session state", async () => {
    seedReadySession();

    const { root, container } = render(
      <AuthGate>
        <div data-testid="protected-child">wallet data</div>
      </AuthGate>,
    );
    await flushEffects();

    expect(container.textContent).toContain("wallet data");

    act(() => {
      usePortfolioStore.getState().resetSession();
    });
    await flushEffects();

    expect(container.textContent).not.toContain("wallet data");
    expect(container.textContent).not.toContain("Continue with email");
    expect(routerReplace).toHaveBeenCalledWith("/login?next=%2Fwallets");

    act(() => root.unmount());
  });

  it("sends a wallet-ready user with no portfolio back to onboarding on product routes", async () => {
    mockPathname = "/dashboard";
    seedReadySession();

    const { root, container } = render(
      <AuthGate>
        <div data-testid="protected-child">dashboard data</div>
      </AuthGate>,
    );
    await flushEffects();

    expect(container.textContent).not.toContain("dashboard data");

    act(() => {
      usePortfolioStore.getState().setPortfolios([]);
    });
    await flushEffects();

    expect(routerReplace).toHaveBeenCalledWith("/onboarding");
    expect(container.textContent).not.toContain("dashboard data");

    act(() => root.unmount());
  });

  it("lets a half-finished account open wallet recovery surfaces", async () => {
    seedPendingWalletSession();

    const { root, container } = render(
      <AuthGate>
        <div data-testid="protected-child">wallet recovery</div>
      </AuthGate>,
    );
    await flushEffects();

    expect(container.textContent).toContain("wallet recovery");
    expect(routerReplace).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("keeps a half-finished account out of product routes", async () => {
    mockPathname = "/dashboard";
    seedPendingWalletSession();

    const { root, container } = render(
      <AuthGate>
        <div data-testid="protected-child">dashboard data</div>
      </AuthGate>,
    );
    await flushEffects();

    expect(container.textContent).not.toContain("dashboard data");
    expect(routerReplace).toHaveBeenCalledWith("/onboarding");

    act(() => root.unmount());
  });
});

function seedReadySession() {
  act(() => {
    usePortfolioStore.getState().setSessionResolved(true);
    usePortfolioStore.getState().setSessionActive(true);
    usePortfolioStore.getState().setWallet({
      walletId: "wallet-1",
      arcAddress: "0x1111111111111111111111111111111111111111",
      baseAddress: "0x2222222222222222222222222222222222222222",
      createdAt: new Date().toISOString(),
    });
  });
}

function seedPendingWalletSession() {
  act(() => {
    usePortfolioStore.getState().setSessionResolved(true);
    usePortfolioStore.getState().setSessionActive(true);
    usePortfolioStore.getState().setWallet(null);
  });
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

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
