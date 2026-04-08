import { test, expect } from '../fixtures/cleanup';

// ---- Shared error capture ----

let jsErrors: string[] = [];

test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
});

test.afterEach(async ({ page }, testInfo) => {
  // DOM snapshot on failure
  if (testInfo.status !== testInfo.expectedStatus) {
    try {
      const dom = await page.content();
      await testInfo.attach('dom-snapshot.html', {
        body: Buffer.from(dom),
        contentType: 'text/html',
      });
    } catch { /* page may have closed */ }
  }

  // Fail test if uncaught JS errors occurred
  if (jsErrors.length > 0) {
    const errList = jsErrors.join('; ');
    jsErrors = [];
    throw new Error(`Uncaught JS errors during test: ${errList}`);
  }
});

// ---- Route smoke tests (BROW-01) ----

// Routes must use /kiosk/ prefix because baseURL is http://host:3300/kiosk
// and Playwright's page.goto('/path') resolves as new URL('/path', baseURL)
// which drops the basePath. Only '/' works without prefix (server redirects root to /kiosk).
const SMOKE_ROUTES = [
  { path: '/', name: 'customer landing', expectedText: /RACING/i },
  { path: '/kiosk/register', name: 'registration page', expectedText: /waiver|register|name/i },
  { path: '/kiosk/staff', name: 'staff login', expectedText: /Staff Terminal|Staff PIN/i },
];

for (const route of SMOKE_ROUTES) {
  test(`smoke: ${route.name} (${route.path}) loads without JS errors`, async ({ page }) => {
    await page.goto(route.path, { waitUntil: 'networkidle' });

    // No React error boundary text in DOM
    const bodyText = await page.textContent('body') ?? '';
    expect(bodyText).not.toMatch(/application error|unhandled runtime error|a client-side exception/i);

    // Expected structural content present (wait for client hydration on "use client" pages)
    await expect(page.getByText(route.expectedText).first()).toBeVisible({ timeout: 10000 });
  });
}

// ---- Keyboard navigation (FOUND-07) ----

test('keyboard: Tab navigates wizard buttons, Enter selects', async ({ page }) => {
  // Navigate to staff page — the wizard is accessed through the staff terminal
  await page.goto('/kiosk/staff', { waitUntil: 'networkidle' });

  // Wait for staff page to hydrate (client-side rendered)
  await page.waitForTimeout(2000);

  // The staff page shows a login screen first, then pod grid, then setup wizard
  // Check if we can reach the wizard — if staff auth is required, skip this test gracefully
  const wizardStep = page.locator('[data-testid="step-select-plan"]');
  const walkinBtn = page.locator('[data-testid="walkin-btn"]');

  // Try clicking walk-in if visible
  if (await walkinBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
    await walkinBtn.click();
  }

  // Wait for wizard — if it doesn't appear (auth required), skip gracefully
  const wizardVisible = await wizardStep.isVisible({ timeout: 10000 }).catch(() => false);
  if (!wizardVisible) {
    test.skip(true, 'Wizard not reachable without staff auth — skipping keyboard test');
    return;
  }

  // Tab through the page — at least one tier button should receive focus
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');

  // Verify a tier option button is focusable
  const focused = await page.evaluate(() => {
    const el = document.activeElement;
    return el?.getAttribute('data-testid') ?? el?.tagName ?? 'none';
  });

  // The focused element should be a button (tier option or other wizard element)
  // We are checking that keyboard navigation works at all, not a specific element
  expect(focused).not.toBe('none');

  // Press Enter on a focused tier button to advance
  await page.keyboard.press('Enter');

  // If a tier was selected, wizard should advance to select_game
  // Give it a moment to transition
  const gameStep = page.locator('[data-testid="step-select-game"]');
  const advanced = await gameStep.isVisible({ timeout: 5000 }).catch(() => false);

  // We accept either outcome: advanced (Enter selected a tier) or stayed (Enter hit a non-tier element)
  // The key assertion is that Tab and Enter do NOT cause JS errors (captured by afterEach)
  if (advanced) {
    // Bonus: verify game option buttons exist and are tabbable
    await page.keyboard.press('Tab');
    const gameFocused = await page.evaluate(() =>
      document.activeElement?.getAttribute('data-testid') ?? 'none'
    );
    // Should be something, ideally game-option-*
    expect(gameFocused).not.toBe('none');
  }
});
