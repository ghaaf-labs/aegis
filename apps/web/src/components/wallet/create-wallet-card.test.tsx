import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { CreateWalletCard } from "./create-wallet-card";
import { walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

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
    readiness: vi.fn(),
    me: vi.fn(),
    status: vi.fn(),
    requestCode: vi.fn(),
    login: vi.fn(),
    create: vi.fn(),
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
  mockSearchParams = new URLSearchParams();
  usePortfolioStore.getState().resetSession();
  window.localStorage.clear();
});

describe("<CreateWalletCard />", () => {
  it("keeps the email form closed while the server session check is pending", async () => {
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockReturnValue(new Promise(() => {}));

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();

    expect(container.textContent).toContain("Checking current session");
    expect(container.textContent).toContain("No code request yet");
    expect(container.querySelector('[data-testid="wallet-auth-email"]')).toBe(
      null,
    );
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );
    expect(walletApi.requestCode).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("fails closed when the auth readiness probe cannot be verified", async () => {
    vi.mocked(walletApi.readiness).mockRejectedValue(new Error("api down"));
    vi.mocked(walletApi.me).mockRejectedValue(new Error("missing token"));

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();

    expect(container.textContent).toContain("Auth check failed");
    expect(container.textContent).toContain("stale browser state");
    expect(container.textContent).toContain("Email form locked");
    expect(container.textContent).toContain("Recheck backend auth capability");
    expect(container.querySelector('[data-testid="wallet-auth-email"]')).toBe(
      null,
    );
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );

    act(() => root.unmount());
  });

  it("locks the login form when the server already verifies this browser", async () => {
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockResolvedValue({
      id: "user-1",
      email: "verified@example.com",
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

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();
    await flushEffects();

    expect(container.textContent).toContain(
      "Already signed in - login form locked",
    );
    expect(container.textContent).toContain("verified@example.com");
    expect(container.textContent).toContain("Fresh code");
    expect(container.textContent).toContain(
      "will not open the app from an existing cookie",
    );
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );
    expect(
      container.querySelector('[data-testid="wallet-auth-existing-continue"]'),
    ).toBe(null);
    expect(walletApi.requestCode).not.toHaveBeenCalled();
    expect(routerReplace).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("locks the signup form when the server already verifies this browser", async () => {
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockResolvedValue({
      id: "user-1",
      email: "signed-in@example.com",
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

    const { root, container } = render(<CreateWalletCard />);
    await flushEffects();
    await flushEffects();

    expect(container.textContent).toContain("Active wallet session found");
    expect(container.textContent).toContain("signed-in@example.com");
    expect(container.textContent).toContain(
      "Signup is blocked while this browser is signed in",
    );
    expect(container.textContent).toContain(
      "will not create or open another wallet",
    );
    expect(container.querySelector('[data-testid="wallet-auth-email"]')).toBe(
      null,
    );
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );
    expect(walletApi.requestCode).not.toHaveBeenCalled();
    expect(routerReplace).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("locks the login form even when wallet status is unavailable", async () => {
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockResolvedValue({
      id: "user-1",
      email: "status-failed@example.com",
      riskTolerance: "moderate",
    });
    vi.mocked(walletApi.status).mockRejectedValue(new Error("circle down"));

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();
    await flushEffects();

    expect(container.textContent).toContain(
      "Session active - login form locked",
    );
    expect(container.textContent).toContain("status-failed@example.com");
    expect(container.textContent).toContain(
      "wallet status could not be verified",
    );
    expect(container.textContent).toContain("Wallet statusunknown");
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );
    expect(walletApi.requestCode).not.toHaveBeenCalled();

    act(() => root.unmount());
  });

  it("explains real-mode auth lock instead of leaving a dead login form", async () => {
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: false,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockRejectedValue(new Error("missing token"));

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();

    expect(container.textContent).toContain("Real auth is blocked");
    expect(container.textContent).toContain(
      "not a wrong email or a hidden browser session",
    );
    expect(container.textContent).toContain("Circle modereal Circle");
    expect(container.textContent).toContain("Email sendermissing");
    expect(container.textContent).toContain("RESEND_API_KEY");
    expect(container.textContent).toContain("Recheck backend auth capability");
    expect(container.textContent).toContain("Email form locked");
    expect(container.textContent).toContain(
      "hidden until the backend can deliver a real one-time code",
    );
    expect(container.querySelector('[data-testid="wallet-auth-email"]')).toBe(
      null,
    );
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );

    act(() => root.unmount());
  });

  it("does not prefill login from remembered local email hints", async () => {
    window.localStorage.setItem("aegis_email", "remembered@example.com");
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockRejectedValue(new Error("missing token"));

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();

    const emailInput = container.querySelector<HTMLInputElement>(
      '[data-testid="wallet-auth-email"]',
    );
    expect(emailInput?.value).toBe("");
    expect(container.textContent).toContain(
      "stale browser hints are not used to fill this field",
    );

    act(() => root.unmount());
  });

  it("treats signedOut=1 plus a still-valid cookie as a failed logout", async () => {
    mockSearchParams = new URLSearchParams("signedOut=1");
    window.localStorage.setItem("aegis_email", "remembered@example.com");
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockResolvedValue({
      id: "user-1",
      email: "still-active@example.com",
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

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();
    await flushEffects();

    expect(window.localStorage.getItem("aegis_email")).toBe(
      "still-active@example.com",
    );
    expect(container.textContent).toContain("Logout did not finish");
    expect(container.textContent).toContain("SESSION STILL ACTIVE");
    expect(container.textContent).toContain("Retry server logout");
    expect(container.querySelector('[data-testid="wallet-auth-submit"]')).toBe(
      null,
    );
    expect(container.querySelector('[data-testid="wallet-auth-email"]')).toBe(
      null,
    );

    act(() => root.unmount());
  });

  it("clears remembered email hints after the server rejects an old session", async () => {
    mockSearchParams = new URLSearchParams("reason=session_expired");
    window.localStorage.setItem("aegis_email", "stale@example.com");
    vi.mocked(walletApi.readiness).mockResolvedValue({
      circleMock: false,
      emailDeliveryConfigured: true,
      devCodesEnabled: false,
    });
    vi.mocked(walletApi.me).mockRejectedValue(new Error("expired"));

    const { root, container } = render(<CreateWalletCard loginMode />);
    await flushEffects();

    const emailInput = container.querySelector<HTMLInputElement>(
      '[data-testid="wallet-auth-email"]',
    );
    expect(window.localStorage.getItem("aegis_email")).toBe(null);
    expect(emailInput?.value).toBe("");
    expect(container.textContent).toContain("Session not accepted");
    expect(container.textContent).toContain("fresh one-time code is required");

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
