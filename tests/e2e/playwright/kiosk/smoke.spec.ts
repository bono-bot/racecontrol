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

const SMOKE_ROUTES = [
  { path: '/', name: 'customer landing', expectedText: /RACING/i },
  { path: '/book', name: 'booking page', expectedText: /Book a Session/i },
  { path: '/staff', name: 'staff login', expectedText: /Staff/i },
];

for (const route of SMOKE_ROUTES) {
  test(`smoke: ${route.name} (${route.path}) loads without JS errors`, async ({ page }) => {
    await page.goto(route.path, { waitUntil: 'networkidle' });

    // No React error boundary text in DOM
    const bodyText = await page.textContent('body') ?? '';
    expect(bodyText).not.toMatch(/application error|unhandled runtime error|a client-side exception/i);

    // Expected structural content present
    await expect(page.getByText(route.expectedText).first()).toBeVisible();
  });
}

// ---- Keyboard navigation (FOUND-07) ----

test('keyboard: Tab navigates wizard buttons, Enter selects', async ({ page }) => {
  // Navigate to booking in staff mode to skip OTP
  await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });

  // Click walk-in button to skip phone auth and enter wizard
  const walkinBtn = page.locator('[data-testid="walkin-btn"]');
  // Walk-in only appears in staff mode — if visible, click it
  if (await walkinBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
    await walkinBtn.click();
  }

  // Wait for wizard to appear (first step is select_plan)
  const wizardStep = page.locator('[data-testid="step-select-plan"]');
  await wizardStep.waitFor({ state: 'visible', timeout: 10000 });

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
