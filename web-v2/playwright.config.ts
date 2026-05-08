import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config — PACT-20260506-001 Phase 1 wire-up Session 6d (Lane A).
 *
 * Scope: E2E coverage for the POS .130 customer-lookup surface
 * (`/v2/pos/lookup` page). Specs at tests/e2e/cirs-lookup.spec.ts.
 *
 * Backend: specs use `page.route()` to mock `/api/v1/cirs/lookup` so the
 * Next.js dev server runs standalone — no racecontrol Rust backend required
 * for this lane. Real backend integration covered by Rust integration tests
 * (cargo test -p racecontrol-crate --lib api::cirs_lookup) and Session 8
 * deploy-time verification on POS .130.
 *
 * Server: webServer block spins up `npm run dev` on port 3500 (the web-v2
 * dev host). reuseExistingServer keeps re-runs fast in development.
 *
 * NOT TESTED via this lane: production Next.js build path (`npm run build`
 * + `npm start`) — dev mode only; visual regression / screenshot diff;
 * cross-browser (chromium only); racecontrol API contract drift (covered
 * by Rust serde round-trip tests at cirs_lookup.rs).
 */
export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:3500",
    trace: "retain-on-failure",
    actionTimeout: 10_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "npm run dev",
    url: "http://localhost:3500/v2/pos/lookup",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
