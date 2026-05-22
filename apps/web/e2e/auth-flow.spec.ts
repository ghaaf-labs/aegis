import { test, expect } from "@playwright/test";
import {
  API_BASE,
  createVerifiedAccount,
  requireDevCodes,
} from "./helpers/auth";

test.beforeEach(async () => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
  if (!(await requireDevCodes())) {
    test.skip(true, "wallet auth UI e2e requires MOCK_CIRCLE dev codes");
  }
});

async function seedPortfolio(token: string) {
  const res = await fetch(`${API_BASE}/portfolios`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      name: "Auth Flow Portfolio",
      allocations: [{ symbol: "USDC", quantity: 0, targetWeight: 100 }],
      goal: {
        name: "Auth Flow Portfolio",
        horizon: "1y",
        riskTolerance: "moderate",
        targetAllocation: { USDC: 100 },
        includeUsyc: false,
        includeEurc: false,
        createdAt: new Date().toISOString(),
      },
    }),
  });
  if (!res.ok) {
    throw new Error(`portfolio seed failed: ${res.status} ${await res.text()}`);
  }
}

test("AUTH1 — login restores an existing user and reaches dashboard", async ({
  page,
}) => {
  const email = `login-${Date.now()}-${Math.random().toString(16).slice(2)}@aegis.local`;
  const { token } = await createVerifiedAccount(email);
  await seedPortfolio(token);

  await page.goto("/login");
  await page.locator('[data-testid="wallet-auth-email"]').fill(email);
  await page.locator('[data-testid="wallet-auth-submit"]').click();
  const devCodeText = await page.getByText(/Mock dev code:/i).textContent();
  const code = devCodeText?.match(/\b\d{6}\b/)?.[0];
  expect(code).toBeTruthy();
  await page.locator('[data-testid="wallet-auth-code"]').fill(code!);
  await page.locator('[data-testid="wallet-auth-submit"]').click();
  await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });
});

test("AUTH2 — signup with an existing wallet refuses silent session restore", async ({
  page,
}) => {
  const email = `existing-${Date.now()}-${Math.random().toString(16).slice(2)}@aegis.local`;
  const { token } = await createVerifiedAccount(email);
  await seedPortfolio(token);

  await page.goto("/signup");
  await page.locator('[data-testid="wallet-auth-email"]').fill(email);
  await page.locator('[data-testid="wallet-auth-submit"]').click();
  const devCodeText = await page.getByText(/Mock dev code:/i).textContent();
  const code = devCodeText?.match(/\b\d{6}\b/)?.[0];
  expect(code).toBeTruthy();
  await page.locator('[data-testid="wallet-auth-code"]').fill(code!);
  await page.locator('[data-testid="wallet-auth-submit"]').click();
  await expect(
    page.getByText(/This email already has a wallet/i),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: /Sign in with this email/i }),
  ).toBeVisible();
});
