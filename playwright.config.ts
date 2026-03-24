import { defineConfig, devices } from '@playwright/test';

/**
 * Multi-project Playwright config for RaceControl E2E.
 *
 * Projects:
 *   - pos-dashboard: Web dashboard at :3200 (staff/admin facing)
 *   - kiosk:         Kiosk app at :3300 (customer + staff facing on pods)
 *
 * Run all:       npx playwright test
 * Run one:       npx playwright test --project=pos-dashboard
 * View report:   npx playwright show-report
 */
export default defineConfig({
  testDir: './tests/e2e/playwright',
  outputDir: './tests/e2e/results',
  fullyParallel: false,
  retries: process.env.CI ? 2 : 1,
  workers: 1,
  reporter: [
    ['html', { open: 'never', outputFolder: 'tests/e2e/report' }],
    ['junit', { outputFile: 'test-results/junit.xml' }],
    ['list'],
  ],
  use: {
    trace: 'on-first-retry',
    screenshot: 'on',
    video: 'retain-on-failure',
    actionTimeout: 10_000,
    navigationTimeout: 15_000,
  },
  projects: [
    {
      name: 'pos-dashboard',
      testDir: './tests/e2e/playwright/pos',
      use: {
        ...devices['Desktop Chrome'],
        baseURL: process.env.POS_BASE_URL ?? 'http://192.168.31.23:3200',
      },
    },
    {
      name: 'kiosk',
      testDir: './tests/e2e/playwright/kiosk',
      use: {
        ...devices['Desktop Chrome'],
        baseURL: process.env.KIOSK_BASE_URL ?? 'http://192.168.31.23:3300/kiosk',
      },
    },
  ],
});
