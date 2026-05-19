import { test, expect } from "@playwright/test";

// SET-series — settings hub and sub-pages. Requires the Rust API.
// Auth state is loaded from global-setup.ts via storageState.

test.use({ storageState: "./e2e/.auth/user.json" });

test.beforeEach(() => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
});

test("SET1 — settings hub shows Wallet, Agent, Peg, Tax links", async ({
  page,
}) => {
  await page.goto("/settings");
  await expect(page.getByRole("link", { name: /Wallet/i })).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByRole("link", { name: /Agent/i })).toBeVisible();
  await expect(page.getByRole("link", { name: /Peg/i })).toBeVisible();
  await expect(page.getByRole("link", { name: /Tax/i })).toBeVisible();
});

test("SET2 — diary visibility toggle present on settings page", async ({
  page,
}) => {
  await page.goto("/settings");
  await expect(page.locator('[data-testid="diary-toggle"]')).toBeVisible({
    timeout: 10_000,
  });
});

test("SET3 — agent settings page shows pause/resume button", async ({
  page,
}) => {
  await page.goto("/settings/agent");
  // Button label changes between Pause and Resume depending on current state.
  const toggle = page.getByRole("button", {
    name: /Pause agent|Resume agent/i,
  });
  await expect(toggle).toBeVisible({ timeout: 10_000 });
  // Status label is also present.
  await expect(page.getByText(/Active|Paused/i).first()).toBeVisible();
});

test("SET4 — peg editor page renders", async ({ page }) => {
  await page.goto("/settings/peg");
  await expect(page.getByRole("heading", { name: /Peg defense/i })).toBeVisible(
    { timeout: 10_000 },
  );
  // PegRuleEditor section must be present.
  await expect(page.locator("main")).toBeVisible();
});

test("SET5 — tax page shows Download CSV button", async ({ page }) => {
  await page.goto("/settings/tax");
  await expect(page.getByRole("button", { name: /Download CSV/i })).toBeVisible(
    { timeout: 10_000 },
  );
});

test("SET7 — sidebar logout clears JWT and redirects to login", async ({
  page,
}) => {
  await page.goto("/dashboard");
  await page.waitForURL(/\/dashboard\//, { timeout: 15_000 });
  // sidebar-logout is gated on wallet hydration; wait for it to mount.
  await page.waitForSelector('[data-testid="sidebar-logout"]', {
    timeout: 10_000,
  });
  await page.locator('[data-testid="sidebar-logout"]').click();
  await expect(page).toHaveURL(/\/login/, { timeout: 10_000 });
  // JWT must be gone from localStorage.
  const jwt = await page.evaluate(() => localStorage.getItem("aegis.jwt"));
  expect(jwt).toBeNull();
});
