import { test, expect } from "@playwright/test";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

test.beforeEach(() => {
  if (!process.env.PLAYWRIGHT_API_ENABLED) test.skip();
});

async function createAccount(email: string): Promise<string> {
  const res = await fetch(`${API_BASE}/auth/wallet/create`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email }),
  });
  if (!res.ok) {
    throw new Error(`auth create failed: ${res.status} ${await res.text()}`);
  }
  const body = (await res.json()) as { token: string };
  return body.token;
}

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
  const token = await createAccount(email);
  await seedPortfolio(token);

  await page.goto("/login");
  await page.locator('input[type="email"]').fill(email);
  await page.getByRole("button", { name: /Sign in/i }).click();
  await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });
});

test("AUTH2 — signup with an existing email signs in instead of duplicating onboarding", async ({
  page,
}) => {
  const email = `existing-${Date.now()}-${Math.random().toString(16).slice(2)}@aegis.local`;
  const token = await createAccount(email);
  await seedPortfolio(token);

  await page.goto("/signup");
  await page.locator('input[type="email"]').fill(email);
  await page.getByRole("button", { name: /Continue/i }).click();
  await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });
});
