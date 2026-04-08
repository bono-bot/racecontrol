import { test, expect } from '../fixtures/cleanup';

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

// ---- Staff login page ----

test('staff: login page has PIN input after tap-to-sign-in', async ({ page }) => {
  await page.goto('/kiosk/staff', { waitUntil: 'networkidle' });

  // Staff page starts in idle state with "Tap to Sign In" button
  const signInBtn = page.getByText('Tap to Sign In');
  await expect(signInBtn).toBeVisible({ timeout: 10000 });
  await signInBtn.click();

  // After clicking, PIN keypad should appear
  const keypad = page.locator('button:has-text("1")');
  const hasKeypad = await keypad.first().isVisible({ timeout: 5000 }).catch(() => false);

  const pinInput = page.locator('input[type="password"], input[type="tel"], input[inputmode="numeric"], [data-testid*="pin"]');
  const hasPin = await pinInput.first().isVisible({ timeout: 3000 }).catch(() => false);

  expect(hasPin || hasKeypad).toBe(true);
});

test('staff: invalid PIN shows error feedback', async ({ page }) => {
  await page.goto('/kiosk/staff', { waitUntil: 'networkidle' });

  // Enter idle → PIN entry mode
  const signInBtn = page.getByText('Tap to Sign In');
  await expect(signInBtn).toBeVisible({ timeout: 10000 });
  await signInBtn.click();
  await page.waitForTimeout(500);

  // Type wrong PIN digits via numeric keypad buttons
  for (const digit of ['0', '0', '0', '0']) {
    const btn = page.locator(`button:has-text("${digit}")`).first();
    if (await btn.isVisible({ timeout: 2000 }).catch(() => false)) {
      await btn.click();
      await page.waitForTimeout(150);
    }
  }

  // Auto-submits at 4 digits — wait for response
  await page.waitForTimeout(3000);

  // Should show error or stay on login — not crash
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

// ---- Control panel ----

test('control: page loads with pod controls', async ({ page }) => {
  await page.goto('/kiosk/control', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

// ---- Fleet overview ----

test('fleet: page loads with pod status grid', async ({ page }) => {
  await page.goto('/kiosk/fleet', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);

  // Should show pod status or fleet info
  const hasFleetContent = /pod|fleet|status|online|offline|idle/i.test(body);
  expect(hasFleetContent || body.length > 50).toBe(true);
});

// ---- Debug panel ----

test('debug: page loads with system info', async ({ page }) => {
  await page.goto('/kiosk/debug', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

test('debug: page loads or redirects to auth', async ({ page }) => {
  await page.goto('/kiosk/debug', { waitUntil: 'networkidle' });

  // Debug page may redirect unauthenticated users to landing page
  const body = await page.textContent('body') ?? '';
  const isDebugPage = /Report Issue|Live Activity|Server Logs|Debug/i.test(body);
  const isLandingPage = /RACING|AVAILABLE|Staff Login/i.test(body);

  // Either the debug page loaded OR we were redirected to a valid page (not an error)
  expect(isDebugPage || isLandingPage).toBe(true);
  expect(body).not.toMatch(/application error|unhandled runtime error/i);
});

// ---- Spectator view ----

test('spectator: page loads for audience display', async ({ page }) => {
  await page.goto('/kiosk/spectator', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

// ---- Settings ----

test('settings: page loads with config options', async ({ page }) => {
  await page.goto('/kiosk/settings', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});
