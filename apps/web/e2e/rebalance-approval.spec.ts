import { test, expect, type Page } from "@playwright/test";
import {
  API_BASE,
  authCookie,
  createVerifiedAccount,
  requireDevCodes,
} from "./helpers/auth";

// R-series — rebalance approval modal + execution trace. Requires the Rust
// API with EXECUTION_MOCK=true and MOCK_CIRCLE=true. The global-setup creates
// a test user; we trigger a plan via the API and visit the resulting URL.

test.use({ storageState: "./e2e/.auth/user.json" });

test.beforeEach(async () => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
  if (!(await requireDevCodes())) {
    test.skip(true, "rebalance e2e account setup requires mock dev codes");
  }
});

/** Create a portfolio + trigger a rebalance plan via the API.
 *  Returns the planId so tests can navigate to /rebalance/{planId}. */
async function seedPlan(): Promise<{ planId: string; jwt: string }> {
  const jwt = await createTestJwt();
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
  return { planId: plan.rebalanceId, jwt };
}

async function createTestJwt(): Promise<string> {
  const email = `rebalance-${Date.now()}-${Math.random().toString(16).slice(2)}@aegis.local`;
  const { token } = await createVerifiedAccount(email);
  return token;
}

async function openSeededPlan(page: Page): Promise<string> {
  const { planId, jwt } = await seedPlan();
  await page.context().clearCookies();
  await page.context().addCookies([authCookie(jwt)]);
  await page.goto(`/rebalance/${planId}`);
  return planId;
}

test("R4 — approval page shows leg cards", async ({ page }) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await page.getByRole("button", { name: /Technical route/i }).click();
  await expect(page.locator('[data-testid="leg-card"]').first()).toBeVisible();
});

test("R5 — approval page shows model badge", async ({ page }) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  const badge = page
    .locator(".font-mono")
    .filter({ hasText: /claude|gpt|haiku|opus|sonnet/i });
  await expect(badge.first()).toBeVisible({ timeout: 10_000 });
});

test("R6 — approval page shows USDC fee estimate", async ({ page }) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByText(/USDC/i).first()).toBeVisible();
});

test("R6b — mock execution mode is explicit", async ({ page }) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  const modal = page.locator('[data-testid="approval-modal"]');
  await expect(modal.getByText(/Local demo execution/i)).toBeVisible();
  await expect(
    modal.getByText(/no real chain transaction is sent/i),
  ).toBeVisible();
  await expect(page.getByText(/Real on-chain execution/i)).toHaveCount(0);
});

test("R7 — Approve button is present and enabled", async ({ page }) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  const approveBtn = page.getByRole("button", {
    name: /Approve|Run local execution/i,
  });
  await expect(approveBtn).toBeVisible();
  await expect(approveBtn).toBeEnabled();
});

test("R8 — clicking Approve shows execution trace", async ({ page }) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await page
    .getByRole("button", { name: /Approve|Run local execution/i })
    .click();
  await expect(page.locator('[data-testid="execution-trace"]')).toBeVisible({
    timeout: 15_000,
  });
});

test("R9 — local execution completes and refreshes dashboard state", async ({
  page,
}) => {
  await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await page
    .getByRole("button", { name: /Approve|Run local execution/i })
    .click();
  await expect(page.locator('[data-testid="execution-trace"]')).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByText(/COMPLETED/i)).toBeVisible({ timeout: 20_000 });
  await expect(page.getByText(/Dashboard updated/i)).toBeVisible({
    timeout: 20_000,
  });
});

test("R11 — Close button navigates away from rebalance page", async ({
  page,
}) => {
  const planId = await openSeededPlan(page);
  await expect(page.locator('[data-testid="approval-modal"]')).toBeVisible({
    timeout: 15_000,
  });
  await page.getByRole("button", { name: /Close|×/i }).click();
  await expect(page).not.toHaveURL(new RegExp(`/rebalance/${planId}`));
});
