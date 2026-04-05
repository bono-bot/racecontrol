import { defineConfig, devices } from '@playwright/test';

const SERVER_IP = '192.168.31.23';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 300_000, // 5 min per test (billing + game launch + verification + cleanup)
  expect: { timeout: 15_000 },
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: 'test-results/html' }],
    ['json', { outputFile: 'test-results/results.json' }],
  ],
  use: {
    screenshot: 'on',
    video: 'on',
    trace: 'retain-on-failure',
    ...devices['Desktop Chrome'],
  },
  outputDir: 'test-results/artifacts',

  projects: [
    {
      name: 'pos-billing',
      use: {
        baseURL: `http://${SERVER_IP}:3200`,
      },
      testMatch: [
        'tests/00-smoke/**',
        'tests/01-buttons/pos-buttons.spec.ts',
        'tests/02-spelling/pos-spelling.spec.ts',
        'tests/03-drivers/**',
        'tests/04-matrix/**',
        'tests/06-coupons/**',
        'tests/08-chaos/pos-chaos.spec.ts',
        'tests/09-financial/**',
      ],
    },
    {
      name: 'staff-kiosk',
      use: {
        baseURL: `http://${SERVER_IP}:3300`,
      },
      testMatch: [
        'tests/00-smoke/kiosk-smoke.spec.ts',
        'tests/01-buttons/kiosk-buttons.spec.ts',
        'tests/02-spelling/kiosk-spelling.spec.ts',
        'tests/05-pause/**',
        'tests/07-multiplayer/**',
        'tests/08-chaos/kiosk-chaos.spec.ts',
      ],
    },
  ],
});
