import { test, expect } from "@playwright/test";
import {
  authCookie,
  createVerifiedAccount,
  requireDevCodes,
} from "./helpers/auth";

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
    page.getByRole("button", {
      name: /Create wallet|Continue|Sign up|Signup email unavailable/i,
    }),
  ).toBeVisible();
});

test("S3 — goal wizard step 1 renders name input and Next button", async ({
  page,
}) => {
  if (!(await requireDevCodes())) {
    test.skip(true, "onboarding e2e account setup requires mock dev codes");
  }
  // The onboarding page mounts the GoalWizard. Use a real verified cookie
  // session, but do not seed a portfolio so the wizard remains visible.
  const email = `onboarding-${Date.now()}-${Math.random().toString(16).slice(2)}@aegis.local`;
  const { token } = await createVerifiedAccount(email);
  await page.context().addCookies([authCookie(token)]);
  await page.goto("/onboarding");
  await expect(page.locator('[data-testid="goal-wizard-step-1"]')).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByRole("button", { name: /Next/i })).toBeVisible();
});

// S4 and S5 need the real storageState from global-setup (a seeded API user).
test.describe("goal wizard steps 4+ (authed)", () => {
  test.use({ storageState: "./e2e/.auth/user.json" });
  test.beforeEach(async () => {
    if (!(await requireDevCodes())) {
      test.skip(true, "authed onboarding e2e requires mock dev codes");
    }
  });

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
