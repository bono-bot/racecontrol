/**
 * Playwright config for the page crawler.
 *
 * Dedicated config separate from the main E2E test suite.
 * Serial execution (workers: 1) to avoid overwhelming the apps.
 *
 * Usage:
 *   npx playwright test --config tests/page-crawler/playwright.config.ts
 */

import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: process.env.PROBE_SPEC ?? 'crawl.spec.ts',
  outputDir: '../../test-results/page-crawler',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    screenshot: 'off', // we handle screenshots manually
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
  },
  projects: [
    {
      name: 'web',
      use: {
        baseURL: process.env.WEB_URL ?? 'http://192.168.31.23:3200',
      },
    },
    {
      name: 'admin',
      use: {
        baseURL: process.env.ADMIN_URL ?? 'http://192.168.31.23:3201',
      },
    },
    {
      name: 'kiosk',
      use: {
        baseURL: process.env.KIOSK_URL ?? 'http://192.168.31.23:3300',
      },
    },
  ],
});
