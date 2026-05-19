import type { Page } from "@playwright/test";

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
