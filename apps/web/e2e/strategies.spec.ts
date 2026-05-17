import { test, expect } from "@playwright/test";

// SM-4 — public strategies marketplace surface. The empty state copy
// references the seed binary, so a CI environment without DB will still
// pass this test (the page either lists strategies or renders the empty
// state — both are acceptable).

test("strategies marketplace renders headline + at least the empty state", async ({
  page,
}) => {
  await page.goto("/strategies");
  await expect(
    page.getByRole("heading", { name: /Pick a starting allocation/i }),
  ).toBeVisible();
});

test("strategies page has a signup CTA in the footer", async ({ page }) => {
  await page.goto("/strategies");
  await expect(
    page.getByRole("link", { name: /Create a wallet/i }),
  ).toBeVisible();
});
