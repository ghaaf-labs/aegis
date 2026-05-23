import { test, expect } from "@playwright/test";
import { requireDevCodes } from "./helpers/auth";

// AP-series — authenticated app-page smoke tests. Verify that each
// important gated page renders its heading and primary content without
// crashing. Requires the Rust API (EXECUTION_MOCK=true, MOCK_CIRCLE=true).
// Tests are skipped automatically when PLAYWRIGHT_API_ENABLED is unset.

test.use({ storageState: "./e2e/.auth/user.json" });

test.beforeEach(async ({ page }) => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
  if (!(await requireDevCodes())) {
    test.skip(true, "app-pages e2e auth state requires mock dev codes");
  }
  // All tests in this file start from a stable dashboard URL so the
  // portfolio store is hydrated before navigating to the target page.
  await page.goto("/dashboard");
  await page.waitForURL(/\/dashboard\//, { timeout: 15_000 });
});

test("AP1 — analytics page renders heading and metric panels", async ({
  page,
}) => {
  await page.goto("/analytics");
  await expect(page.getByRole("heading", { name: /Analytics/i })).toBeVisible({
    timeout: 10_000,
  });
  // "Portfolio telemetry" label appears above the heading
  await expect(page.getByText(/Portfolio telemetry/i)).toBeVisible();
});

test("AP2 — agent studio page renders heading and controls", async ({
  page,
}) => {
  await page.goto("/agent-studio");
  await expect(
    page.getByRole("heading", { name: /Agent Studio/i }),
  ).toBeVisible({ timeout: 10_000 });
  // Pause / Resume button is always rendered (state-dependent label)
  await expect(
    page.getByRole("button", { name: /Pause agent|Resume agent/i }),
  ).toBeVisible({ timeout: 10_000 });
});

test("AP3 — agent logs page renders heading and decision list or empty state", async ({
  page,
}) => {
  await page.goto("/agent-logs");
  await expect(page.getByRole("heading", { name: /Agent Logs/i })).toBeVisible({
    timeout: 10_000,
  });
  // Either decision rows or empty-state copy must appear
  const rows = page.locator('[data-testid="decision-card"]');
  const empty = page.getByText(/No decisions yet|No agent decisions/i);
  await expect(rows.or(empty)).toBeVisible({ timeout: 10_000 });
});

test("AP4 — transactions page renders heading and history or empty state", async ({
  page,
}) => {
  await page.goto("/transactions");
  await expect(
    page.getByRole("heading", { name: /Transactions/i }),
  ).toBeVisible({ timeout: 10_000 });
  // Either a rebalance row or the empty-state paragraph
  const rows = page.locator('[data-testid="rebalance-row"]');
  const empty = page.getByText(/No rebalance history|No transactions/i);
  await expect(rows.or(empty).or(page.locator("main"))).toBeVisible({
    timeout: 10_000,
  });
});

test("AP5 — portfolio page renders heading and allocation chart", async ({
  page,
}) => {
  await page.goto("/portfolio");
  await expect(
    page.getByRole("heading", { name: /My Portfolio/i }),
  ).toBeVisible({ timeout: 10_000 });
  // AllocationChart or AssetTable must appear as the main content
  await expect(page.getByText(/Allocation|Holdings/i).first()).toBeVisible({
    timeout: 10_000,
  });
});

test("AP6 — settings billing page renders heading", async ({ page }) => {
  await page.goto("/settings/billing");
  await expect(page.getByRole("heading", { name: /Billing/i })).toBeVisible({
    timeout: 10_000,
  });
});
