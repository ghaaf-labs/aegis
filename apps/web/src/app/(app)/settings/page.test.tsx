import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

import SettingsIndex from "./page";

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
    ...props
  }: React.AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("@/lib/api", () => ({
  accountApi: {
    deleteAccount: vi.fn(),
    exportData: vi.fn(),
    startEmailUpdate: vi.fn(),
    verifyEmailUpdate: vi.fn(),
  },
  portfolioApi: {
    getDiaryPublic: vi.fn(),
    setDiaryPublic: vi.fn(),
  },
  walletApi: {
    session: vi.fn().mockResolvedValue({
      user: { email: "verified@example.com" },
    }),
  },
}));

vi.mock("@/lib/use-api-query", () => ({
  useApiQuery: () => ({ data: { diaryPublic: false } }),
}));

vi.mock("@/components/settings/digest-opt-in", () => ({
  DigestOptIn: () => <div data-testid="digest-opt-in" />,
}));

vi.mock("@/components/settings/diary-visibility-toggle", () => ({
  DiaryVisibilityToggle: () => <div data-testid="diary-toggle" />,
}));

vi.mock("@/stores/portfolio", () => ({
  useActivePortfolio: () => ({ id: "portfolio-1" }),
  usePortfolioStore: (selector: (state: unknown) => unknown) =>
    selector({
      resetSession: vi.fn(),
      wallet: {
        arcAddress: "0x8955c4848b7e3ce309700b7001caa2c7df50f7f7",
      },
    }),
}));

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
  localStorage.clear();
  vi.clearAllMocks();
});

describe("<SettingsIndex />", () => {
  it("explains why a new email cannot be requested", async () => {
    localStorage.setItem("aegis_email", "verified@example.com");
    const { container, root } = render(<SettingsIndex />);
    await flushEffects();

    const input = newEmailInput(container);
    act(() => {
      setInputValue(input, "bad-email");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });

    expect(container.textContent).toContain("Enter a valid email address.");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      "settings-new-email-validation",
    );
    expect(
      container
        .querySelector("#settings-new-email-validation")
        ?.getAttribute("role"),
    ).toBe("alert");
    expect(container.textContent).not.toContain("Use an email you can access.");
    expect(sendCodeButton(container).disabled).toBe(true);

    act(() => root.unmount());
  });

  it("blocks changing to the current email with a clear reason", async () => {
    localStorage.setItem("aegis_email", "verified@example.com");
    const { container, root } = render(<SettingsIndex />);
    await flushEffects();

    const input = newEmailInput(container);
    act(() => {
      setInputValue(input, "verified@example.com");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });

    expect(container.textContent).toContain("Use a different email address.");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(sendCodeButton(container).disabled).toBe(true);

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

function newEmailInput(container: HTMLElement) {
  const input = container.querySelector<HTMLInputElement>(
    'input[aria-label="New email address"]',
  );
  if (!input) throw new Error("missing new email input");
  return input;
}

function sendCodeButton(container: HTMLElement) {
  const button = Array.from(container.querySelectorAll("button")).find(
    (node) => node.textContent?.trim() === "Send code",
  );
  if (!button) throw new Error("missing send code button");
  return button as HTMLButtonElement;
}

function setInputValue(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  if (!setter) throw new Error("missing input value setter");
  setter.call(input, value);
}

async function flushEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
