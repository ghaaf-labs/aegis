import { type Page, expect } from "@playwright/test";

export const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
export const WEB_BASE =
  process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:3000";
const SESSION_COOKIE_NAME = process.env.SESSION_COOKIE_NAME ?? "aegis_session";

const TEST_JWT =
  process.env.PLAYWRIGHT_TEST_JWT ??
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0LXVzZXIiLCJpYXQiOjE3MDAwMDAwMDB9.fake-sig-for-testing";

/** Legacy-only helper used by the negative auth-gate test. A localStorage JWT
 * must never authenticate the app. */
export async function injectTestJwt(page: Page, jwt = TEST_JWT) {
  await page.addInitScript((token) => {
    localStorage.setItem("aegis.jwt", token);
  }, jwt);
}

export async function clearJwt(page: Page) {
  await page.addInitScript(() => {
    localStorage.removeItem("aegis.jwt");
  });
}

export async function waitForDashboard(page: Page) {
  await page.waitForURL(/\/dashboard\//);
  await expect(page.locator('[data-testid="portfolio-summary"]')).toBeVisible({
    timeout: 10_000,
  });
}

export interface VerifiedAccount {
  email: string;
  token: string;
}

export function authCookie(token: string) {
  const host = new URL(WEB_BASE).hostname;
  return {
    name: SESSION_COOKIE_NAME,
    value: token,
    domain: host,
    path: "/",
    expires: Math.floor(Date.now() / 1000) + 60 * 60,
    httpOnly: true,
    secure: WEB_BASE.startsWith("https://"),
    sameSite: "Lax" as const,
  };
}

export function storageStateForToken(token: string) {
  return {
    cookies: [authCookie(token)],
    origins: [],
  };
}

export function sessionCookieHeader(token: string) {
  return `${SESSION_COOKIE_NAME}=${token}`;
}

export async function createVerifiedAccount(
  email: string,
): Promise<VerifiedAccount> {
  return completeWalletAuth(email);
}

export async function loginVerifiedAccount(
  email: string,
): Promise<VerifiedAccount> {
  return completeWalletAuth(email);
}

export async function requireDevCodes() {
  const res = await fetch(`${API_BASE}/auth/email/start`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Aegis-Request": "1",
    },
    body: JSON.stringify({
      email: `probe-${Date.now()}-${Math.random().toString(16).slice(2)}@aegis.local`,
    }),
  });
  if (!res.ok) return false;
  const body = (await res.json()) as { devCode?: string };
  return Boolean(body.devCode);
}

async function completeWalletAuth(email: string): Promise<VerifiedAccount> {
  const codeRes = await fetch(`${API_BASE}/auth/email/start`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Aegis-Request": "1",
    },
    body: JSON.stringify({ email }),
  });
  if (!codeRes.ok) {
    throw new Error(
      `auth code failed: ${codeRes.status} ${await codeRes.text()}`,
    );
  }
  const challenge = (await codeRes.json()) as {
    challengeId: string;
    email: string;
    devCode?: string;
  };
  if (!challenge.devCode) {
    throw new Error("e2e auth requires MOCK_CIRCLE dev verification codes");
  }

  const finishRes = await fetch(`${API_BASE}/auth/email/verify`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Aegis-Request": "1",
    },
    body: JSON.stringify({
      email: challenge.email,
      challengeId: challenge.challengeId,
      code: challenge.devCode,
      consent: {
        tos: true,
        privacy: true,
        tosVersion: "2026-05",
        privacyVersion: "2026-05",
        marketingOptIn: false,
      },
    }),
  });
  if (!finishRes.ok) {
    throw new Error(
      `auth verify failed: ${finishRes.status} ${await finishRes.text()}`,
    );
  }
  return {
    email: challenge.email,
    token: sessionTokenFrom(finishRes),
  };
}

function sessionTokenFrom(response: Response) {
  const setCookie = response.headers.get("set-cookie") ?? "";
  const match = setCookie.match(
    new RegExp(`${SESSION_COOKIE_NAME}=([^;]+)`, "i"),
  );
  if (!match?.[1]) {
    throw new Error("auth response did not set the session cookie");
  }
  return match[1];
}
