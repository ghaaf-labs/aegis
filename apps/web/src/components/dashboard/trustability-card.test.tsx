import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { TrustabilityCard } from "./trustability-card";

const apiMock = vi.hoisted(() => ({
  me: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  trustabilityApi: {
    me: apiMock.me,
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

describe("<TrustabilityCard />", () => {
  it("shows progress diagnostics when the user has plans but no eligible outcomes", async () => {
    apiMock.me.mockResolvedValue({
      row: null,
      label: null,
      progress: {
        calibrationFloor: 50,
        agentDecisions7d: 5,
        eligibleOutcomes7d: 0,
        pendingRealRebalances7d: 1,
        completedRealRebalances7d: 0,
        distinctModels7d: 3,
        lastDecisionAt: "2026-05-24T15:42:32.322Z",
      },
    });

    const { container, root } = render(<TrustabilityCard />);
    await flushEffects();

    const text = container.textContent ?? "";
    expect(text).toContain("0 / 50");
    expect(text).toContain("Execution outcome pending");
    expect(text).toContain("agent plans 7d");
    expect(text).toContain("5");
    expect(text).toContain("pending real");
    expect(text).toContain("1");
    expect(text).toContain("models used");
    expect(text).toContain("3");

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
    await Promise.resolve();
    await Promise.resolve();
  });
}
