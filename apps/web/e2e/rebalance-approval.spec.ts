import { test, expect } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";

// R-series — rebalance approval modal + execution trace. Requires the Rust
// API with EXECUTION_MOCK=true and MOCK_CIRCLE=true. The global-setup creates
// a test user; we trigger a plan via the API and visit the resulting URL.

test.use({ storageState: "./e2e/.auth/user.json" });

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

test.beforeEach(() => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
});

/** Read the JWT saved by global-setup from the storageState file. */
function savedJwt(): string {
  try {
    const raw = fs.readFileSync(
      path.join(__dirname, ".auth", "user.json"),
      "utf-8",
    );
    const state = JSON.parse(raw) as {
      origins?: Array<{
        localStorage?: Array<{ name: string; value: string }>;
      }>;
    };
    return (
      state.origins
        ?.flatMap((o) => o.localStorage ?? [])
        .find((e) => e.name === "aegis.jwt")?.value ?? ""
    );
  } catch {
    return "";
  }
}

/** Create a portfolio + trigger a rebalance plan via the API.
 *  Returns the planId so tests can navigate to /rebalance/{planId}. */
async function seedPlan(jwt: string): Promise<string> {
  const pfRes = await fetch(`${API_BASE}/portfolios`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${jwt}`,
    },
    body: JSON.stringify({
      name: "E2E Rebalance Test",
      allocations: [
        { symbol: "BTC", quantity: 0, targetWeight: 60 },
        { symbol: "ETH", quantity: 0, targetWeight: 40 },
      ],
      goal: {
        name: "E2E Rebalance Test",
        horizon: "5y",
        riskTolerance: "moderate",
        targetAllocation: { BTC: 60, ETH: 40 },
        includeUsyc: false,
        includeEurc: false,
        createdAt: new Date().toISOString(),
      },
    }),
  });
  if (!pfRes.ok)
    throw new Error(
      `portfolio create failed: ${pfRes.status} ${await pfRes.text()}`,
    );
  const pf = (await pfRes.json()) as { id: string };

  const planRes = await fetch(
    `${API_BASE}/portfolios/${pf.id}/rebalance/plan`,
    { method: "POST", headers: { Authorization: `Bearer ${jwt}` } },
  );
  if (!planRes.ok)
    throw new Error(
      `plan create failed: ${planRes.status} ${await planRes.text()}`,
    );
  const plan = (await planRes.json()) as { rebalanceId: string };
  return plan.rebalanceId;
}

test("R4 — approval page shows leg cards", async ({ page }) => {
  const planId = await seedPlan(savedJwt());
  await page.goto(`/rebalance/${planId}`);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.locator('[data-testid="leg-card"]').first()).toBeVisible();
});

test("R5 — approval page shows model badge", async ({ page }) => {
  const planId = await seedPlan(savedJwt());
  await page.goto(`/rebalance/${planId}`);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  const badge = page
    .locator(".font-mono")
    .filter({ hasText: /claude|gpt|haiku|opus|sonnet/i });
  await expect(badge.first()).toBeVisible({ timeout: 10_000 });
});

test("R6 — approval page shows USDC fee estimate", async ({ page }) => {
  const planId = await seedPlan(savedJwt());
  await page.goto(`/rebalance/${planId}`);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  // The approval modal shows a fee estimate labelled with "USDC" (Paymaster gas or protocol fee).
  await expect(
    page.locator('[data-testid="approval-modal"]').getByText(/USDC/i).first(),
  ).toBeVisible();
});

test("R7 — Approve button is present and enabled", async ({ page }) => {
  const planId = await seedPlan(savedJwt());
  await page.goto(`/rebalance/${planId}`);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  const approveBtn = page.getByRole("button", { name: /Approve/i });
  await expect(approveBtn).toBeVisible();
  await expect(approveBtn).toBeEnabled();
});

test("R8 — clicking Approve shows execution trace", async ({ page }) => {
  const planId = await seedPlan(savedJwt());
  await page.goto(`/rebalance/${planId}`);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await page.getByRole("button", { name: /Approve/i }).click();
  await expect(page.locator('[data-testid="execution-trace"]')).toBeVisible({
    timeout: 15_000,
  });
});

test("R11 — Close button navigates away from rebalance page", async ({
  page,
}) => {
  const planId = await seedPlan(savedJwt());
  await page.goto(`/rebalance/${planId}`);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  // ApprovalModal calls onClose; the /rebalance/[planId] page routes away.
  await page.getByRole("button", { name: "Close" }).click();
  await expect(page).not.toHaveURL(new RegExp(`/rebalance/${planId}`));
});
