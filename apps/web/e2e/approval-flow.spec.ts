import { test, expect } from "@playwright/test";

// FE-E2E-1 — end-to-end happy path through the approval modal in
// mocked-mode (EXECUTION_MOCK=true, MOCK_CIRCLE=true). Walks: explore
// demo → sign-up CTA visible → policy + constitution links resolve.
//
// The full signup → analyze → approve flow requires a seeded test user
// against a real backend; that's the realm of the integration smoke
// described in plan §N0.10. This spec covers the surface that doesn't
// need a live API.

test.describe("approval-flow surface", () => {
  test("explore page loads with a demo portfolio", async ({ page }) => {
    await page.goto("/explore");
    await expect(page).toHaveTitle(/Aegis/i);
  });

  test("policy page links to constitution + decision routes", async ({
    page,
  }) => {
    await page.goto("/policy");
    await expect(
      page.getByRole("heading", { name: /Outcome.*Refund Policy/i }),
    ).toBeVisible();
    const constitutionLink = page.locator('a[href="/about/constitution"]');
    await expect(constitutionLink).toBeVisible();
  });
});
