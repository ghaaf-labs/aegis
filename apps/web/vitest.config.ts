import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// Vitest config for component + hook tests. JS-DOM environment so we can
// instantiate React components and mock `EventSource` for SSE hook tests.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    // Guarantees a working window.localStorage/sessionStorage under jsdom across
    // Node 20–25 (Node 25's built-in Storage is method-less without a flag).
    setupFiles: ["./src/test-setup.ts"],
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov", "json-summary"],
      reportsDirectory: "./coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.d.ts",
        "src/**/__mocks__/**",
        "src/**/mock-data.ts",
        "src/test-setup.ts",
        "src/app/**", // Next.js route shells are integration-tested, not unit-tested
      ],
    },
  },
});
