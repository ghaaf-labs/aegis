import { test, expect } from "@playwright/test";

// X-series — multi-page cross-link journeys. Require only the Next.js
// dev server (no API backend).

test("X1 — landing → explore → continue CTA flow", async ({ page }) => {
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
  // Click an account CTA from explore
  await page
    .getByRole("link", {
      name: /Continue|Sign up|Create.*(wallet|account)|Get started/i,
    })
    .first()
    .click();
  await expect(page).toHaveURL(/\/login/);
});

test("X2 — demo detail account CTA leads to login page", async ({ page }) => {
  await page.goto("/explore/conservative-retiree");
  await page
    .getByRole("link", {
      name: /Continue|Sign up|Create.*(wallet)|Get started/i,
    })
    .first()
    .click();
  await expect(page).toHaveURL(/\/login/);
});

test("X3 — policy page links to constitution page", async ({ page }) => {
  await page.goto("/policy");
  const constitutionLink = page.getByRole("link", {
    name: "Agent constitution",
  });
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
  // If the leaderboard has entries the diary links appear; if empty, just
  // verify the page is stable (no crash).
  const diaryLink = page.getByRole("link", { name: /^0x[a-f0-9]+/i }).first();
  const hasEntries = (await diaryLink.count()) > 0;
  if (hasEntries) {
    const href = await diaryLink.getAttribute("href");
    await diaryLink.click();
    await expect(page).toHaveURL(/\/diary\//);
    expect(href).toMatch(/^\/diary\//);
  }
});

test("X5 — strategies guest CTA links to login", async ({ page }) => {
  await page.goto("/strategies");
  await expect(
    page.getByRole("heading", { name: /Strategies/i }),
  ).toBeVisible();
  const cta = page.getByRole("link", { name: /Continue with email/i }).first();
  await expect(cta).toBeVisible();
  const href = await cta.getAttribute("href");
  expect(href).toMatch(/\/login/);
});

test("X6 — /signup forwards to /login with preserved query params", async ({
  page,
}) => {
  await page.goto("/signup?ref=affiliate42&next=%2Fonboarding");
  await expect(page).toHaveURL(/\/login\?ref=affiliate42&next=%2Fonboarding/);
});
