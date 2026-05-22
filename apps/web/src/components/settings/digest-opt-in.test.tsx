import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { DigestOptIn } from "./digest-opt-in";

vi.mock("@/lib/api", () => ({
  analyticsApi: {
    track: vi.fn().mockResolvedValue(undefined),
  },
  digestApi: {
    subscribe: vi.fn().mockResolvedValue(undefined),
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
});

describe("<DigestOptIn />", () => {
  it("hydrates the email when the server profile arrives after first render", async () => {
    const { root, container } = render(<DigestOptIn defaultEmail="" />);

    expect(emailInput(container).value).toBe("");
    expect(subscribeButton(container).disabled).toBe(true);

    act(() => root.render(<DigestOptIn defaultEmail="verified@example.com" />));
    await flushEffects();

    expect(emailInput(container).value).toBe("verified@example.com");
    expect(subscribeButton(container).disabled).toBe(false);

    act(() => root.unmount());
  });

  it("does not overwrite an email the user already typed", async () => {
    const { root, container } = render(<DigestOptIn defaultEmail="" />);
    const input = emailInput(container);
    act(() => {
      setInputValue(input, "manual@example.com");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });

    act(() => root.render(<DigestOptIn defaultEmail="verified@example.com" />));
    await flushEffects();

    expect(emailInput(container).value).toBe("manual@example.com");

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

function emailInput(container: HTMLElement) {
  const input = container.querySelector<HTMLInputElement>(
    'input[type="email"]',
  );
  if (!input) throw new Error("missing email input");
  return input;
}

function subscribeButton(container: HTMLElement) {
  const button = Array.from(container.querySelectorAll("button")).find(
    (node) => node.textContent === "Subscribe",
  );
  if (!button) throw new Error("missing subscribe button");
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
