import { test, expect } from "@playwright/test";

// P-series — public pages that require no auth and no API backend.
// All tests in this file must pass with only the Next.js dev server running.

test("P1 — landing page has Explore + Continue + Strategies CTAs", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("link", { name: /Explore/i }).first(),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: /Continue|Get started/i }).first(),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: /Strategies/i }).first(),
  ).toBeVisible();
});

test("P2 — explore index shows demo portfolio cards", async ({ page }) => {
  await page.goto("/explore");
  await expect(
    page.getByRole("heading", { name: /Explore demo portfolios/i }),
  ).toBeVisible();
  // Three demo cards — at least 3 links to specific demo portfolios
  const cards = page.locator('a[href^="/explore/"]');
  await expect(cards).toHaveCount(3, { timeout: 10_000 });
});

test("P3 — demo detail conservative-retiree renders", async ({ page }) => {
  await page.goto("/explore/conservative-retiree");
  await expect(page.getByText(/DEMO PORTFOLIO/i).first()).toBeVisible();
  await expect(page.getByText(/Conservative Retiree/i).first()).toBeVisible();
});

test("P4 — demo detail aggressive-builder renders", async ({ page }) => {
  await page.goto("/explore/aggressive-builder");
  await expect(page.getByText(/DEMO PORTFOLIO/i).first()).toBeVisible();
  await expect(page.getByText(/Aggressive Builder/i).first()).toBeVisible();
});

test("P5 — demo detail operating-reserve renders", async ({ page }) => {
  await page.goto("/explore/operating-reserve");
  await expect(page.getByText(/DEMO PORTFOLIO/i).first()).toBeVisible();
  await expect(page.getByText(/Operating Reserve/i).first()).toBeVisible();
});

test("P6 — leaderboard renders heading or empty state", async ({ page }) => {
  await page.goto("/leaderboard");
  await expect(
    page.getByRole("heading", { name: /Leaderboard/i }),
  ).toBeVisible();
});

test("P7 — diary unknown wallet shows empty state without crashing", async ({
  page,
}) => {
  await page.goto("/diary/unknown-wallet-handle-00000");
  // Diary page should load without crashing — either entries or empty state
  await expect(page.locator("main")).toBeVisible();
  // The wallet address should appear on the page
  await expect(page.getByText(/unknown-wallet-handle-00000/i)).toBeVisible();
});

test("P8 — regime model card page loads", async ({ page }) => {
  await page.goto("/about/regime");
  await expect(
    page.getByRole("heading", { name: /Regime classifier/i }),
  ).toBeVisible();
});

test("P9 — regime backtest page loads or shows empty state", async ({
  page,
}) => {
  await page.goto("/about/regime/backtest");
  // Page should render a heading regardless of whether data exists
  await expect(page.locator("h1, h2").first()).toBeVisible();
});

test("P10 — about page renders hero and team members", async ({ page }) => {
  await page.goto("/about");
  await expect(page.getByRole("heading", { name: /AEGIS/i })).toBeVisible();
  await expect(page.getByText(/Mahdi Zarrintareh/i)).toBeVisible();
  await expect(page.getByText(/Mohammad Jalili/i)).toBeVisible();
  await expect(page.getByText(/Staff Engineer/i).first()).toBeVisible();
});

test("P11 — pricing page renders tiers and heading", async ({ page }) => {
  await page.goto("/pricing");
  await expect(
    page.getByRole("heading", { name: /Stablecoin-native pricing/i }),
  ).toBeVisible();
  // At least the Free tier label appears somewhere on the page
  await expect(page.getByText(/Free forever/i).first()).toBeVisible();
});

test("P12 — help page renders heading and guide sections", async ({ page }) => {
  await page.goto("/help");
  await expect(
    page.getByRole("heading", { name: /Help/i }).first(),
  ).toBeVisible();
  // Descriptive sub-text confirms the real content loaded (not a crash screen)
  await expect(
    page.getByText(/wallet cash|approvals|agent decisions/i).first(),
  ).toBeVisible();
});
