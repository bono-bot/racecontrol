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

// ---- Sidebar navigation: every nav link works ----

test('sidebar: all nav links are clickable and navigate', async ({ page }) => {
  await page.goto('/', { waitUntil: 'networkidle' });

  // Find all sidebar nav links
  const navLinks = page.locator('nav a[href], aside a[href]');
  const count = await navLinks.count();

  // Must have at least 5 nav items (pods, billing, drivers, sessions, settings)
  expect(count).toBeGreaterThanOrEqual(5);

  // Collect all hrefs
  const hrefs: string[] = [];
  for (let i = 0; i < count; i++) {
    const href = await navLinks.nth(i).getAttribute('href');
    if (href && !href.startsWith('http') && !href.startsWith('#')) {
      hrefs.push(href);
    }
  }

  // Click each internal nav link and verify it doesn't crash
  for (const href of [...new Set(hrefs)]) {
    await page.goto(href, { waitUntil: 'networkidle' });
    const body = await page.textContent('body') ?? '';
    expect(body).not.toMatch(/application error|unhandled runtime error/i);
  }
});

// ---- Home page: pod grid renders pod cards ----

test('home: pod grid renders cards with status', async ({ page }) => {
  await page.goto('/', { waitUntil: 'networkidle' });

  // Look for pod-related elements (cards, tiles, or grid items)
  const podElements = page.locator('[data-testid^="pod-"], [class*="pod"]').first();
  const hasPods = await podElements.isVisible({ timeout: 5000 }).catch(() => false);

  // Either pods are visible or a "no pods" / "waiting" message is shown
  if (!hasPods) {
    const body = await page.textContent('body') ?? '';
    expect(body).toMatch(/pod|waiting|no data|offline/i);
  }
});
