import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { BrutalBadge, BrutalButton, BrutalCard, Skeleton } from "@aegis/ui";

beforeAll(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("dashboard brutal primitives", () => {
  it("keep a 2px brutal border alongside border color classes", () => {
    const { container, root } = render(
      <div>
        <BrutalCard data-testid="card">Card</BrutalCard>
        <BrutalButton data-testid="button">Button</BrutalButton>
        <BrutalBadge data-testid="badge">Badge</BrutalBadge>
        <Skeleton data-testid="skeleton" />
      </div>,
    );

    for (const testId of ["card", "button", "badge", "skeleton"]) {
      expect(
        container.querySelector(`[data-testid="${testId}"]`)?.className,
      ).toContain("border-[2px]");
    }

    expect(
      container.querySelector('[data-testid="card"]')?.className,
    ).toContain("border-border-default");
    expect(
      container.querySelector('[data-testid="button"]')?.className,
    ).toContain("border-black");

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
