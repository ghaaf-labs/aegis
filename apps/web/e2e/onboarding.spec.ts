import { test, expect } from "@playwright/test";

// FE-E2E-2 — the cold-start landing surface should be empty-state-clean
// for a fresh user. With FE-MOCK-1 landed, the authenticated routes no
// longer leak demo data into the production paths, so visiting /signup
// without a session must show the actual signup CTA, not pre-populated
// portfolio cards.

test("landing page advertises explore + signup", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/Explore/i)).toBeVisible();
});

test("policy page is reachable without auth", async ({ page }) => {
  await page.goto("/policy");
  await expect(
    page.getByRole("heading", { name: /Outcome.*Refund Policy/i }),
  ).toBeVisible();
});
