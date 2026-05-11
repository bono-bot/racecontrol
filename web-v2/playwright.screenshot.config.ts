import { defineConfig, devices } from "@playwright/test";

/**
 * Screenshot-only playwright config — points at temp standalone server
 * (port 3502) so production pm2 on 3500 stays untouched. Authored
 * 2026-05-11 ~04:00 IST for V2 customer-entry frontpage v0.1 evidence.
 */
export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: /customer-entry-screenshots\.spec\.ts$/,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:3502",
    trace: "off",
    actionTimeout: 10_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
