import { defineConfig, devices } from "@playwright/test";

// FE-E2E-1/FE-E2E-2 — one worker, one browser (chromium), no parallel tests
// within a file so SSE assertions stay deterministic.
//
// Two projects:
//   chromium        — public + P/A/X/ST-series (no auth required)
//   chromium-authed — S/D/R/SET-series (storageState from global-setup.ts)
//
// Global setup runs first. When PLAYWRIGHT_API_ENABLED is unset it writes an
// empty storage-state file and authed tests are skipped by the test files
// themselves (they call test.skip when the API is absent).

export default defineConfig({
  testDir: "./e2e",
  globalSetup: "./e2e/global-setup.ts",
  timeout: 60_000,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:3000",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
      testIgnore: ["**/*.authed.spec.ts"],
    },
    {
      name: "chromium-authed",
      use: {
        ...devices["Desktop Chrome"],
        storageState: "./e2e/.auth/user.json",
      },
      testMatch: ["**/*.authed.spec.ts"],
    },
  ],
  webServer: process.env.PLAYWRIGHT_BASE_URL
    ? undefined
    : {
        command: "pnpm dev",
        url: "http://localhost:3000",
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
});
