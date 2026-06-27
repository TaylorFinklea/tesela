import { defineConfig, devices } from "@playwright/test";

// Functional e2e config (separate from playwright.perf.config.ts). Drives a
// real client against a dev server proxied to a relay-off tesela-server.
// `tests/e2e/run.mjs` stands up the server + dev + a seeded daily and sets the
// env below; you can also point it at an already-running harness via the env.
export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: /.*\.spec\.ts/,
  timeout: 45_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: process.env.TESELA_E2E_BASE_URL ?? "http://127.0.0.1:5199",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
