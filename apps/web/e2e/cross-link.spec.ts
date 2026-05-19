import { test, expect } from "@playwright/test";

// X-series — multi-page cross-link journeys. Require only the Next.js
// dev server (no API backend).

test("X1 — landing → explore → signup CTA flow", async ({ page }) => {
  await page.goto("/");
  // Navigate to explore
  await page
    .getByRole("link", { name: /Explore demo/i })
    .first()
    .click();
  await expect(page).toHaveURL(/\/explore/);
  await expect(
    page.getByRole("heading", { name: /Explore demo portfolios/i }),
  ).toBeVisible();
  // Click a signup CTA from explore
  await page
    .getByRole("link", {
      name: /Sign up|Create.*(wallet|account)|Get started/i,
    })
    .first()
    .click();
  await expect(page).toHaveURL(/\/sign(up)?/);
});

test("X2 — demo detail signup CTA leads to signup page", async ({ page }) => {
  await page.goto("/explore/conservative-retiree");
  await page
    .getByRole("link", { name: /Sign up|Create.*(wallet)|Get started/i })
    .first()
    .click();
  await expect(page).toHaveURL(/\/sign(up)?/);
});

test("X3 — policy page links to constitution page", async ({ page }) => {
  await page.goto("/policy");
  const constitutionLink = page.getByRole("link", { name: /constitution/i });
  await expect(constitutionLink).toBeVisible();
  await constitutionLink.click();
  await expect(page).toHaveURL(/\/about\/constitution/);
  await expect(
    page.getByRole("heading", { name: /constitution/i }),
  ).toBeVisible();
});

test("X4 — leaderboard row links to diary page", async ({ page }) => {
  await page.goto("/leaderboard");
  await expect(
    page.getByRole("heading", { name: /Leaderboard/i }),
  ).toBeVisible();
  // Diary links only appear when there are leaderboard entries (seeded users).
  // On a fresh DB this block is skipped; the heading assertion above still
  // guards against a crash or broken route.
  const diaryLink = page.locator('a[href^="/diary/"]').first();
  if ((await diaryLink.count()) > 0) {
    const href = await diaryLink.getAttribute("href");
    await diaryLink.click();
    await expect(page).toHaveURL(/\/diary\//);
    expect(href).toMatch(/^\/diary\//);
  }
});

test("X5 — strategies guest CTA links to signup", async ({ page }) => {
  await page.goto("/strategies");
  await expect(
    page.getByRole("heading", { name: /Strategies/i }),
  ).toBeVisible();
  // Footer or in-page CTA should link to signup
  await expect(
    page.getByRole("link", { name: /Create a wallet/i }),
  ).toBeVisible();
  const href = await page
    .getByRole("link", { name: /Create a wallet/i })
    .first()
    .getAttribute("href");
  expect(href).toMatch(/\/sign(up)?/);
});

test("X6 — signup page with ?ref= param loads without error", async ({
  page,
}) => {
  await page.goto("/signup?ref=testhandle");
  await expect(page).toHaveURL(/\/signup/);
  // Page should render without crashing — ref param is silently consumed
  await expect(page.locator("main, body")).toBeVisible();
  await expect(
    page.locator('[data-testid="auth-gate-message"]'),
  ).not.toBeVisible();
});
