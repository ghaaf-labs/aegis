import { test, expect } from "@playwright/test";

// FE-E2E-2 — /decision/<id> public route. Without a backed-up decision
// row the page renders the "Decision not found" fallback; that fallback
// must offer a recovery link back to the leaderboard. This guards the
// FE-ERR-1 / FE-TRACE-1 combined contract: the page never crashes, it
// either shows the audit trail or a graceful 404-style empty state.

test("decision trace shows a graceful empty state for unknown ids", async ({
  page,
}) => {
  await page.goto("/decision/00000000-0000-0000-0000-000000000000");
  await expect(page.getByText(/Decision not found/i)).toBeVisible();
  await expect(page.getByRole("link", { name: /leaderboard/i })).toBeVisible();
});

test("about page renders team and hero", async ({ page }) => {
  await page.goto("/about");
  await expect(page.getByRole("heading", { name: /AEGIS/i })).toBeVisible();
  await expect(page.getByText(/Mahdi Zarrintareh/i)).toBeVisible();
  await expect(page.getByText(/Mohammad Jalili/i)).toBeVisible();
});
