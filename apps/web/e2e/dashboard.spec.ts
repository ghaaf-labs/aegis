import { test, expect } from "@playwright/test";

// D-series — dashboard + wallet surfaces. Requires the Rust API running
// with EXECUTION_MOCK=true and MOCK_CIRCLE=true. Auth state is injected
// by global-setup.ts (PLAYWRIGHT_API_ENABLED=true).
//
// Tests are skipped when no API is available (storageState file exists but
// is empty — global-setup writes {} when PLAYWRIGHT_API_ENABLED is unset).

test.use({ storageState: "./e2e/.auth/user.json" });

test.beforeEach(async ({ page }) => {
  // Skip gracefully when running without the API.
  if (!process.env.PLAYWRIGHT_API_ENABLED) {
    test.skip();
  }
  // Wait for the dashboard to redirect to an actual portfolio page.
  await page.goto("/dashboard");
  await page.waitForURL(/\/dashboard\//, { timeout: 15_000 });
});

test("D1 — portfolio summary card visible on dashboard", async ({ page }) => {
  await expect(page.locator('[data-testid="portfolio-summary"]')).toBeVisible({
    timeout: 10_000,
  });
});

test("D2 — deploy prompt shown when wallet has idle funds", async ({
  page,
}) => {
  // In mock mode the gateway always returns ~100 USDC, so the "Deploy wallet
  // balance" prompt (showDeploy) appears instead of the faucet (showFaucet).
  await expect(
    page.getByRole("button", { name: /Deploy wallet balance/i }),
  ).toBeVisible({
    timeout: 10_000,
  });
});

test("D5 — agent reasoning feed renders or shows empty state", async ({
  page,
}) => {
  // A fresh user has no decisions so the empty state copy appears.
  const emptyFeed = page.getByText(/No decisions yet|No agent decisions/i);
  // If populated, a decision card is present. Either is acceptable.
  const decisionCard = page.locator('[data-testid="decision-card"]');
  await expect(emptyFeed.or(decisionCard)).toBeVisible({ timeout: 10_000 });
});

test("D7 — sidebar nav items are all present", async ({ page }) => {
  await expect(page.getByRole("link", { name: /Dashboard/i })).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Wallet", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: /Portfolio/i })).toBeVisible();
  await expect(page.getByRole("link", { name: /Strategies/i })).toBeVisible();
  await expect(page.getByRole("link", { name: /Settings/i })).toBeVisible();
});

test("D8 — wallet page shows per-chain balance rows", async ({ page }) => {
  await page.goto("/wallet");
  await expect(page.getByText(/Arc Testnet/i)).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText(/Base Sepolia/i)).toBeVisible();
});

test("D9 — idle cash card is visible on dashboard", async ({ page }) => {
  await expect(page.locator('[data-testid="idle-cash-card"]')).toBeVisible({
    timeout: 10_000,
  });
});

test("D10 — allocation chart section renders", async ({ page }) => {
  // AllocationChart is inside a card — look for the section heading.
  const chartHeading = page.getByText(/Allocation|Target allocation/i).first();
  await expect(chartHeading).toBeVisible({ timeout: 10_000 });
});
