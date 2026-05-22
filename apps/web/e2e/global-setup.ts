import { type FullConfig } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import {
  API_BASE,
  createVerifiedAccount,
  loginVerifiedAccount,
  requireDevCodes,
  sessionCookieHeader,
  storageStateForToken,
} from "./helpers/auth";

const AUTH_STATE_PATH = path.join(__dirname, ".auth", "user.json");
const TEST_EMAIL = "e2e-test@aegis.local";

/**
 * Global setup — runs once before all tests.
 *
 * When PLAYWRIGHT_API_ENABLED=true, this completes the same two-step
 * verification-code auth flow the UI uses, then stores the HttpOnly cookie in
 * e2e/.auth/user.json so S/D/R/SET-series tests exercise real cookie auth.
 *
 * When the env var is absent, the file is written with no cookies so authed
 * tests are skipped gracefully.
 */
export default async function globalSetup(_config: FullConfig) {
  const authDir = path.dirname(AUTH_STATE_PATH);
  if (!fs.existsSync(authDir)) fs.mkdirSync(authDir, { recursive: true });

  if (!process.env.PLAYWRIGHT_API_ENABLED) {
    // Write empty storage state so storageState references don't error.
    fs.writeFileSync(
      AUTH_STATE_PATH,
      JSON.stringify({ cookies: [], origins: [] }),
    );
    return;
  }

  // Wait for the API to be ready (up to 120s).
  await waitForApi();
  if (!(await requireDevCodes())) {
    fs.writeFileSync(
      AUTH_STATE_PATH,
      JSON.stringify({ cookies: [], origins: [] }),
    );
    return;
  }

  let token: string;
  try {
    ({ token } = await createVerifiedAccount(TEST_EMAIL));
  } catch {
    ({ token } = await loginVerifiedAccount(TEST_EMAIL));
  }

  // Seed a portfolio so D-series tests land on /dashboard/<id> rather than
  // being redirected to /onboarding (which happens when no portfolio exists).
  await seedPortfolio(token);

  fs.writeFileSync(
    AUTH_STATE_PATH,
    JSON.stringify(storageStateForToken(token)),
  );
}

async function seedPortfolio(token: string): Promise<void> {
  // Idempotent — if a portfolio already exists this will create a second one,
  // which is fine; the dashboard redirects to the first it finds.
  const res = await fetch(`${API_BASE}/portfolios`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Aegis-Request": "1",
      Cookie: sessionCookieHeader(token),
    },
    body: JSON.stringify({
      name: "E2E Test Portfolio",
      allocations: [{ symbol: "USDC", quantity: 0, targetWeight: 100 }],
      goal: {
        name: "E2E Test Portfolio",
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
    // Non-fatal — D-series tests will just skip if /dashboard redirects to /onboarding.
    console.warn(`[global-setup] portfolio seed failed: ${res.status}`);
  }
}

async function waitForApi(retries = 60, delayMs = 2_000): Promise<void> {
  for (let i = 0; i < retries; i++) {
    try {
      const r = await fetch(`${API_BASE}/health`);
      if (r.ok) return;
    } catch {
      /* not ready yet */
    }
    await new Promise((r) => setTimeout(r, delayMs));
  }
  throw new Error(`API at ${API_BASE} did not become ready in time`);
}
