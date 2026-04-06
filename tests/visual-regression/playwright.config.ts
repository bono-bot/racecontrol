/**
 * Playwright config for visual regression tests.
 *
 * Dedicated config separate from the main E2E test suite and page-crawler.
 * Serial execution (workers: 1) to avoid overwhelming the apps.
 * Snapshots stored in __screenshots__/ for git-committed baselines.
 *
 * Usage:
 *   npx playwright test --config tests/visual-regression/playwright.config.ts
 *   npx playwright test --config tests/visual-regression/playwright.config.ts --update-snapshots
 */

import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'visual.spec.ts',
  outputDir: '../../test-results/visual-regression',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  snapshotPathTemplate: '{testDir}/__screenshots__/{projectName}/{testFilePath}/{arg}{ext}',
  expect: {
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.01,
      threshold: 0.2,
    },
  },
  use: {
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
