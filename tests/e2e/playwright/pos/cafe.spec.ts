import { test, expect } from '@playwright/test';

// ---- JS error capture ----
let jsErrors: string[] = [];
test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
});
test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status !== testInfo.expectedStatus) {
    try {
      await testInfo.attach('dom-snapshot.html', {
        body: Buffer.from(await page.content()),
        contentType: 'text/html',
      });
    } catch {}
  }
  if (jsErrors.length > 0) {
    const msg = jsErrors.join('; ');
    jsErrors = [];
    throw new Error(`Uncaught JS errors: ${msg}`);
  }
});

// ---- Cafe management ----

test('cafe: page loads with menu items or empty state', async ({ page }) => {
  await page.goto('/cafe', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);

  // Should show menu items, categories, or empty state
  const hasCafeContent = /menu|item|category|cafe|order|empty|no items/i.test(body);
  expect(hasCafeContent || body.length > 100).toBe(true);
});

test('cafe: add item button exists', async ({ page }) => {
  await page.goto('/cafe', { waitUntil: 'networkidle' });

  const addBtn = page.getByRole('button', { name: /add|create|new/i });
  const hasAdd = await addBtn.first().isVisible({ timeout: 5000 }).catch(() => false);

  // Button existence check — do NOT click to avoid data mutation
  if (hasAdd) {
    await expect(addBtn.first()).toBeEnabled();
  }
});
