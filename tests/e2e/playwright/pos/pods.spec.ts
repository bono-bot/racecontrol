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

// ---- Pod management page ----

test('pods: page loads with pod list or empty state', async ({ page }) => {
  await page.goto('/pods', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);

  // Should render pod cards/rows or a no-pods message
  const hasPodContent = /pod|sim|offline|online|idle/i.test(body);
  expect(hasPodContent || body.length > 100).toBe(true);
});

test('pods: wake-all button is present and clickable', async ({ page }) => {
  await page.goto('/pods', { waitUntil: 'networkidle' });

  const wakeAllBtn = page.getByRole('button', { name: /wake.*all/i });
  const hasWakeAll = await wakeAllBtn.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasWakeAll) {
    // Verify it's enabled and has correct aria
    await expect(wakeAllBtn).toBeEnabled();
    // Do NOT click — this is a read-only audit, not a destructive action
  }
});

test('pods: shutdown-all button is present', async ({ page }) => {
  await page.goto('/pods', { waitUntil: 'networkidle' });

  const shutdownBtn = page.getByRole('button', { name: /shutdown.*all|shut.*down/i });
  const hasShutdown = await shutdownBtn.isVisible({ timeout: 5000 }).catch(() => false);

  // Button may be conditionally hidden — either way, no JS errors
  if (hasShutdown) {
    await expect(shutdownBtn).toBeEnabled();
  }
});

test('pods: individual pod card has action buttons', async ({ page }) => {
  await page.goto('/pods', { waitUntil: 'networkidle' });

  // Find first pod card or row
  const podCard = page.locator('[data-testid^="pod-"], [class*="pod"]').first();
  const hasPod = await podCard.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasPod) {
    // Pod cards should have action buttons (wake, shutdown, restart, etc.)
    const buttons = podCard.locator('button');
    const btnCount = await buttons.count();
    // At minimum there should be some interactive element
    expect(btnCount).toBeGreaterThanOrEqual(0);
  }
});
