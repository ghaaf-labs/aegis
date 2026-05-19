import { chromium, type FullConfig } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
const AUTH_STATE_PATH = path.join(__dirname, ".auth", "user.json");
const TEST_EMAIL = "e2e-test@aegis.local";

/**
 * Global setup — runs once before all tests.
 *
 * When PLAYWRIGHT_API_ENABLED=true (set by the CI job that starts the Rust
 * API), this creates a test user via POST /auth/wallet/create and saves the
 * JWT + browser storage state to e2e/.auth/user.json so S/D/R/SET-series
 * tests can use it via storageState.
 *
 * When the env var is absent (frontend-only runs), the file is written with
 * an empty localStorage so authed tests are skipped gracefully.
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

  // Wait for the API to be ready (up to 30s).
  await waitForApi();

  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();

  // Sign up (or re-login if the test user already exists from a previous run).
  const res = await fetch(`${API_BASE}/auth/wallet/create`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email: TEST_EMAIL }),
  });

  if (!res.ok) {
    // Fall back to login if the user was already created.
    const loginRes = await fetch(`${API_BASE}/auth/wallet/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: TEST_EMAIL }),
    });
    if (!loginRes.ok) throw new Error(`Auth setup failed: ${loginRes.status}`);
    const { token } = (await loginRes.json()) as { token: string };
    await injectToken(page, token);
  } else {
    const { token } = (await res.json()) as { token: string };
    await injectToken(page, token);
  }

  await context.storageState({ path: AUTH_STATE_PATH });
  await browser.close();
}

async function injectToken(
  page: import("@playwright/test").Page,
  token: string,
) {
  // Navigate to app root so localStorage is scoped to the right origin.
  await page.goto(process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:3000", {
    waitUntil: "domcontentloaded",
  });
  await page.evaluate((t) => localStorage.setItem("aegis.jwt", t), token);
}

async function waitForApi(retries = 15, delayMs = 2_000): Promise<void> {
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
