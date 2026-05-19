import { test, expect } from "@playwright/test";
import { injectTestJwt } from "./helpers/auth";

// A-series — auth gate behaviour. All tests pass with only the Next.js
// dev server (no API backend required).

const GATED_ROUTES = ["/dashboard", "/portfolio", "/wallet", "/settings"];

for (const route of GATED_ROUTES) {
  test(`A1-A4 — unauthenticated ${route} shows wallet gate`, async ({
    page,
  }) => {
    await page.goto(route);
    await expect(page.getByText(/Create a wallet to continue/i)).toBeVisible();
    await expect(
      page.getByRole("link", { name: /Create wallet/i }),
    ).toBeVisible();
  });
}

test("A5 — explore is accessible without JWT", async ({ page }) => {
  await page.goto("/explore");
  await expect(
    page.getByRole("heading", { name: /Explore demo portfolios/i }),
  ).toBeVisible();
  await expect(
    page.getByText(/Create a wallet to continue/i),
  ).not.toBeVisible();
});

test("A6 — leaderboard is accessible without JWT", async ({ page }) => {
  await page.goto("/leaderboard");
  await expect(
    page.getByRole("heading", { name: /Leaderboard/i }),
  ).toBeVisible();
  await expect(
    page.getByText(/Create a wallet to continue/i),
  ).not.toBeVisible();
});

test("A7 — JWT in localStorage bypasses auth gate on dashboard", async ({
  page,
}) => {
  await injectTestJwt(page);
  await page.goto("/dashboard");
  // Gate message must not appear — shell renders
  await expect(
    page.getByText(/Create a wallet to continue/i),
  ).not.toBeVisible();
});
