import { defineConfig, devices } from "@playwright/test";

// FE-E2E-1/FE-E2E-2 — one worker, one browser (chromium), no parallel tests
// within a file so SSE assertions stay deterministic.
//
// Single chromium project. Authed specs (D/R/SET-series) declare
// test.use({ storageState }) at file scope; the storageState file is written
// by global-setup.ts. When PLAYWRIGHT_API_ENABLED is unset, global-setup
// writes an empty file and the authed specs call test.skip() in beforeEach.

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
