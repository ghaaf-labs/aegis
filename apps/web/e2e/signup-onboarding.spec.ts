import { test, expect } from "@playwright/test";

// S-series — signup form surface + goal wizard. Requires the Rust API
// with MOCK_CIRCLE=true. These tests use a fresh (unauthenticated) browser
// context — no storageState — so they can exercise the sign-up flow.

test.beforeEach(() => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
});

test("S1 — signup page renders email input and submit button", async ({
  page,
}) => {
  await page.goto("/signup");
  await expect(page.locator('input[type="email"]')).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Create wallet|Continue|Sign up/i }),
  ).toBeVisible();
});

test("S3 — goal wizard step 1 renders name input and Next button", async ({
  page,
}) => {
  // The onboarding page mounts the GoalWizard — visit it with a JWT so the
  // auth gate doesn't block, but without a portfolio so the wizard shows.
  // Pass the JWT value outside the callback; process.env is not available
  // inside the serialized browser script.
  const jwt =
    process.env.PLAYWRIGHT_TEST_JWT ??
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0LXVzZXIiLCJpYXQiOjE3MDAwMDAwMDB9.fake-sig-for-testing";
  await page.addInitScript((token) => {
    localStorage.setItem("aegis.jwt", token);
  }, jwt);
  await page.goto("/onboarding");
  await expect(page.locator('[data-testid="goal-wizard-step-1"]')).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByRole("button", { name: /Next/i })).toBeVisible();
});

// S4 and S5 need the real storageState from global-setup (a seeded API user).
test.describe("goal wizard steps 4+ (authed)", () => {
  test.use({ storageState: "./e2e/.auth/user.json" });

  test("S4 — goal wizard step 4 renders after navigating through steps 1-3", async ({
    page,
  }) => {
    await page.goto("/onboarding");
    await expect(
      page.locator('[data-testid="goal-wizard-step-1"]'),
    ).toBeVisible({ timeout: 10_000 });
    await page.locator('input[type="text"]').first().fill("Test Portfolio");
    await page.getByRole("button", { name: /Next/i }).click();
    await expect(
      page.locator('[data-testid="goal-wizard-step-2"]'),
    ).toBeVisible();
    await page.getByRole("button", { name: /Next/i }).click();
    await expect(
      page.locator('[data-testid="goal-wizard-step-3"]'),
    ).toBeVisible();
    await page.getByRole("button", { name: /Next/i }).click();
    await expect(
      page.locator('[data-testid="goal-wizard-step-4"]'),
    ).toBeVisible();
    // Default allocation sums to 100 — Create portfolio should be visible.
    await expect(
      page.getByRole("button", { name: /Create portfolio/i }),
    ).toBeVisible();
  });

  test("S5 — goal wizard step 4 enables submit when allocation = 100%", async ({
    page,
  }) => {
    await page.goto("/onboarding");
    await expect(
      page.locator('[data-testid="goal-wizard-step-1"]'),
    ).toBeVisible({ timeout: 10_000 });
    await page.locator('input[type="text"]').first().fill("My Portfolio");
    await page.getByRole("button", { name: /Next/i }).click();
    await expect(
      page.locator('[data-testid="goal-wizard-step-2"]'),
    ).toBeVisible();
    await page.getByRole("button", { name: /Next/i }).click();
    await expect(
      page.locator('[data-testid="goal-wizard-step-3"]'),
    ).toBeVisible();
    await page.getByRole("button", { name: /Next/i }).click();
    await expect(
      page.locator('[data-testid="goal-wizard-step-4"]'),
    ).toBeVisible();
    // Default allocation is 100% — Create portfolio should be enabled.
    const submit = page.getByRole("button", { name: /Create portfolio/i });
    await expect(submit).toBeEnabled();
  });
});
