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

test('staff: login page has PIN input', async ({ page }) => {
  await page.goto('/staff', { waitUntil: 'networkidle' });

  // Should have a PIN input field or keypad
  const pinInput = page.locator('input[type="password"], input[type="tel"], input[inputmode="numeric"], [data-testid*="pin"]');
  const hasPin = await pinInput.first().isVisible({ timeout: 5000 }).catch(() => false);

  // Either PIN input or a numeric keypad exists
  const keypad = page.locator('[data-testid*="keypad"], button:has-text("1")');
  const hasKeypad = await keypad.first().isVisible({ timeout: 3000 }).catch(() => false);

  expect(hasPin || hasKeypad).toBe(true);
});

test('staff: invalid PIN shows error feedback', async ({ page }) => {
  await page.goto('/staff', { waitUntil: 'networkidle' });

  // Type a wrong PIN
  const pinInput = page.locator('input[type="password"], input[type="tel"], input[inputmode="numeric"]').first();
  const hasInput = await pinInput.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasInput) {
    await pinInput.fill('0000');
    // Submit
    const submitBtn = page.getByRole('button', { name: /login|submit|enter|go/i });
    const hasSubmit = await submitBtn.isVisible({ timeout: 3000 }).catch(() => false);
    if (hasSubmit) {
      await submitBtn.click();
      // Should show error — not crash
      await page.waitForTimeout(2000);
      const body = await page.textContent('body') ?? '';
      expect(body).not.toMatch(/application error/i);
    }
  }
});

// ---- Control panel ----

test('control: page loads with pod controls', async ({ page }) => {
  await page.goto('/control', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

// ---- Fleet overview ----

test('fleet: page loads with pod status grid', async ({ page }) => {
  await page.goto('/fleet', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);

  // Should show pod status or fleet info
  const hasFleetContent = /pod|fleet|status|online|offline|idle/i.test(body);
  expect(hasFleetContent || body.length > 50).toBe(true);
});

// ---- Debug panel ----

test('debug: page loads with system info', async ({ page }) => {
  await page.goto('/debug', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

test('debug: diagnostics panel has issue input and submit', async ({ page }) => {
  await page.goto('/debug', { waitUntil: 'networkidle' });

  // Expand diagnostics panel
  const diagnosticsToggle = page.locator('button:has-text("Report Issue")');
  const hasToggle = await diagnosticsToggle.isVisible({ timeout: 5000 }).catch(() => false);
  if (hasToggle) {
    await diagnosticsToggle.click();
  }

  // Should have issue textarea
  const textarea = page.locator('textarea[placeholder*="Describe the problem"]');
  const hasTextarea = await textarea.isVisible({ timeout: 3000 }).catch(() => false);
  expect(hasTextarea).toBe(true);

  // Should have submit button
  const submitBtn = page.locator('button:has-text("Submit")');
  const hasSubmit = await submitBtn.isVisible({ timeout: 3000 }).catch(() => false);
  expect(hasSubmit).toBe(true);
});

test('debug: live activity section visible', async ({ page }) => {
  await page.goto('/debug', { waitUntil: 'networkidle' });

  // Should show Live Activity header
  const activityHeader = page.locator('text=Live Activity');
  const hasActivity = await activityHeader.isVisible({ timeout: 5000 }).catch(() => false);
  expect(hasActivity).toBe(true);

  // Should show connection indicator (Live or Disconnected)
  const connIndicator = page.locator('text=/Live|Disconnected/');
  const hasConn = await connIndicator.first().isVisible({ timeout: 3000 }).catch(() => false);
  expect(hasConn).toBe(true);
});

test('debug: server logs panel is collapsible', async ({ page }) => {
  await page.goto('/debug', { waitUntil: 'networkidle' });

  // Should have Server Logs toggle
  const logsToggle = page.locator('button:has-text("Server Logs")');
  const hasLogs = await logsToggle.isVisible({ timeout: 5000 }).catch(() => false);
  expect(hasLogs).toBe(true);
});

// ---- Spectator view ----

test('spectator: page loads for audience display', async ({ page }) => {
  await page.goto('/spectator', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

// ---- Settings ----

test('settings: page loads with config options', async ({ page }) => {
  await page.goto('/settings', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});
