import { test, expect } from "@playwright/test";
import { injectTestJwt } from "./helpers/auth";

// A-series — auth gate behaviour. All tests pass with only the Next.js
// dev server (no API backend required).

const GATED_ROUTES = [
  "/dashboard",
  "/portfolio",
  "/wallets",
  "/settings",
  "/wallet",
  "/agent-studio",
  "/agent-logs",
  "/analytics",
  "/transactions",
  "/tax-center",
  "/settings/billing",
];

for (const route of GATED_ROUTES) {
  test(`A1-A4 — unauthenticated ${route} redirects to verified login`, async ({
    page,
  }) => {
    await page.goto(route);
    await expect(page).toHaveURL(/\/login\?next=/);
    await expect(page.getByText(/Enter your email to continue/i)).toBeVisible();
    await expect(page.getByRole("button", { name: "Continue" })).toBeVisible();
  });
}

test("A5 — help page is accessible without JWT", async ({ page }) => {
  await page.goto("/help");
  await expect(
    page.getByRole("heading", { name: /Help/i }).first(),
  ).toBeVisible();
  await expect(page).not.toHaveURL(/\/login/);
});

test("A6 — explore is accessible without JWT", async ({ page }) => {
  await page.goto("/explore");
  await expect(
    page.getByRole("heading", { name: /Explore demo portfolios/i }),
  ).toBeVisible();
  await expect(page.getByText(/Sign in to continue/i)).not.toBeVisible();
});

test("A7 — leaderboard is accessible without JWT", async ({ page }) => {
  await page.goto("/leaderboard");
  await expect(
    page.getByRole("heading", { name: /Leaderboard/i }),
  ).toBeVisible();
  await expect(page.getByText(/Sign in to continue/i)).not.toBeVisible();
});

test("A8 — fake JWT in localStorage does not bypass auth gate", async ({
  page,
}) => {
  await injectTestJwt(page);
  await page.goto("/dashboard");
  await expect(page).toHaveURL(/\/login\?next=/);
  await expect(page.getByText(/Enter your email to continue/i)).toBeVisible();
});
