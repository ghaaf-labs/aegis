// NOTE: injectTestJwt uses a client-side decoy JWT (fake signature).
// It bypasses the AuthGate localStorage check only — server-side JWT_SECRET
// validation will 401. Do NOT use for tests that call the API (D/R/SET-series).
// Those tests must rely on the real storageState written by global-setup.ts.

import { type Page, expect } from "@playwright/test";

const TEST_JWT =
  process.env.PLAYWRIGHT_TEST_JWT ??
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0LXVzZXIiLCJpYXQiOjE3MDAwMDAwMDB9.fake-sig-for-testing";

export async function injectTestJwt(page: Page, jwt = TEST_JWT) {
  await page.addInitScript((token) => {
    localStorage.setItem("aegis.jwt", token);
  }, jwt);
}

export async function clearJwt(page: Page) {
  await page.addInitScript(() => {
    localStorage.removeItem("aegis.jwt");
  });
}

export async function waitForDashboard(page: Page) {
  await page.waitForURL(/\/dashboard\//);
  await expect(page.locator('[data-testid="portfolio-summary"]')).toBeVisible({
    timeout: 10_000,
  });
}
