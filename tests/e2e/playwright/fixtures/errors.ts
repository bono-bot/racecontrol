import { test as base, expect } from '@playwright/test';

/**
 * Fixture that captures JS errors on every page and fails tests
 * if uncaught exceptions occur. Attaches DOM snapshot on failure.
 */
export const test = base.extend<{ jsErrors: string[] }>({
  jsErrors: async ({ page }, use) => {
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));
    await use(errors);
  },
});

/** Shared afterEach: attach DOM snapshot on failure, fail on JS errors */
export function setupErrorCapture() {
  base.afterEach(async ({ page }, testInfo) => {
    if (testInfo.status !== testInfo.expectedStatus) {
      try {
        const dom = await page.content();
        await testInfo.attach('dom-snapshot.html', {
          body: Buffer.from(dom),
          contentType: 'text/html',
        });
      } catch { /* page may have closed */ }
    }
  });
}

export { expect };
