import { test, expect } from "@playwright/test";
import { injectTestJwt } from "./helpers/auth";

// A-series — auth gate behaviour. All tests pass with only the Next.js
// dev server (no API backend required).

const GATED_ROUTES = [
  { id: "A1", route: "/dashboard" },
  { id: "A2", route: "/portfolio" },
  { id: "A3", route: "/wallet" },
  { id: "A4", route: "/settings" },
];

for (const { id, route } of GATED_ROUTES) {
  test(`${id} — unauthenticated ${route} shows wallet gate`, async ({
    page,
  }) => {
    await page.goto(route);
    await expect(
      page.locator('[data-testid="auth-gate-message"]'),
    ).toBeVisible();
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
    page.locator('[data-testid="auth-gate-message"]'),
  ).not.toBeVisible();
});

test("A6 — leaderboard is accessible without JWT", async ({ page }) => {
  await page.goto("/leaderboard");
  await expect(
    page.getByRole("heading", { name: /Leaderboard/i }),
  ).toBeVisible();
  await expect(
    page.locator('[data-testid="auth-gate-message"]'),
  ).not.toBeVisible();
});

test("A7 — JWT in localStorage bypasses auth gate on dashboard", async ({
  page,
}) => {
  // injectTestJwt uses a fake-signature token that only satisfies the
  // client-side AuthGate (localStorage presence check). Do NOT reuse this
  // helper in D/R/SET-series specs that send requests to the API — the
  // server's JWT_SECRET validation will 401.
  await injectTestJwt(page);
  await page.goto("/dashboard");
  await expect(
    page.locator('[data-testid="auth-gate-message"]'),
  ).not.toBeVisible();
});
