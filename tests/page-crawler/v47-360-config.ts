/**
 * v47.0 Phase 360 dedicated Playwright config for targeted wallet topup
 * screenshot capture. Separate from the main crawler because the main
 * crawler uses `testMatch: 'crawl.spec.ts'` which excludes everything else.
 */
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'crawl-wallet-topup-v47.spec.ts',
  outputDir: '../../test-results/v47-phase-360',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    screenshot: 'off',
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
  },
});
