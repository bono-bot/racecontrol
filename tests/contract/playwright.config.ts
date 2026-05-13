/**
 * Playwright config for tests/contract/ (V2 LBAC contract tests).
 *
 * Used by Layer 1.17 cross-surface-consistency.spec.ts and any future
 * cross-system contract tests that exercise raw HTTP/fetch (no browser).
 *
 * Run:    npx playwright test --config tests/contract/playwright.config.ts
 * List:   npx playwright test --config tests/contract/playwright.config.ts --list
 */
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: /.*\.spec\.ts/,
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [['list']],
});
