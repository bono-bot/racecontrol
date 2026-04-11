import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: 'topup-verify.spec.ts',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    screenshot: 'off',
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
  },
});
