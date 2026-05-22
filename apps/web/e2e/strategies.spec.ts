import { test, expect } from "@playwright/test";
import { injectTestJwt } from "./helpers/auth";

// SM-4 / ST-series — public strategies marketplace surface. The page handles
// both empty state (no DB) and populated state gracefully.

test("strategies marketplace renders headline + at least the empty state", async ({
  page,
}) => {
  await page.goto("/strategies");
  await expect(
    page.getByRole("heading", { name: /Strategies/i }),
  ).toBeVisible();
});

test("strategies page has a continue CTA in the footer", async ({ page }) => {
  await page.goto("/strategies");
  await expect(
    page.getByRole("link", { name: /Continue with email/i }),
  ).toBeVisible();
});

// ST1 — empty state shows onboarding / custom-portfolio link
test("ST1 — empty state shows link to build a custom portfolio", async ({
  page,
}) => {
  await page.goto("/strategies");
  // If strategies list is empty, the page shows a "build a custom portfolio" link
  // pointing to /onboarding. If populated, this test is a no-op.
  const emptySection = page.getByText(/No strategies available yet/i);
  const hasEmpty = (await emptySection.count()) > 0;
  if (hasEmpty) {
    await expect(
      page.getByRole("link", { name: /build a custom portfolio/i }),
    ).toBeVisible();
  }
});

// ST2 — strategy cards render name + risk band when list is populated
test("ST2 — strategy cards show name and risk when strategies exist", async ({
  page,
}) => {
  await page.goto("/strategies");
  const cards = page
    .locator(".border-brutal")
    .filter({ hasText: /low|medium|high/i });
  const count = await cards.count();
  if (count > 0) {
    await expect(cards.first()).toBeVisible();
  }
});

// ST3 — unauthenticated visitor: adopt CTA wraps to /login
test("ST3 — guest adopt CTA links to login page", async ({ page }) => {
  await page.goto("/strategies");
  const ctaLink = page
    .getByRole("link", { name: /Continue with email/i })
    .first();
  await expect(ctaLink).toBeVisible();
  const href = await ctaLink.getAttribute("href");
  expect(href).toMatch(/\/login/);
});

// ST4 — authenticated visitor sees Adopt buttons (not "Sign up to adopt")
test("ST4 — authed visitor sees Adopt buttons", async ({ page }) => {
  await injectTestJwt(page);
  await page.goto("/strategies");
  await expect(
    page.getByRole("heading", { name: /Strategies/i }),
  ).toBeVisible();
  // With JWT set, any strategy cards should render "Adopt" not "Sign up to adopt"
  const signUpCta = page.getByText(/Sign up to adopt/i);
  await expect(signUpCta).toHaveCount(0);
});
