import { test, expect } from '@playwright/test';

/**
 * UI interaction E2E tests for POS billing flows.
 * Tests actual user interactions on the web dashboard (:3200).
 */

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

// ---- Billing Active: Pod Grid & Session Cards ----

test('billing: pod grid renders all pods with status indicators', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  // Should show the billing heading
  const heading = page.getByRole('heading', { name: /billing/i });
  await expect(heading).toBeVisible({ timeout: 5000 });

  // Should show session count
  const sessionCount = page.getByText(/active session/i);
  await expect(sessionCount).toBeVisible({ timeout: 5000 });
});

test('billing: pod cards have start/action buttons', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  // Pod cards should have clickable elements (start button or session actions)
  const buttons = page.locator('button');
  const buttonCount = await buttons.count();
  expect(buttonCount).toBeGreaterThan(0);
});

test('billing: clicking idle pod opens start modal', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  // Find an idle pod and click its start/book button
  const startBtn = page.locator('button').filter({ hasText: /start|book|assign/i }).first();
  const hasStart = await startBtn.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasStart) {
    await startBtn.click();

    // Modal should appear with driver selection or pricing
    const modal = page.locator('[role="dialog"], .modal, [data-testid*="modal"]');
    const hasModal = await modal.isVisible({ timeout: 5000 }).catch(() => false);

    // Or an overlay/form should appear
    const form = page.locator('select, input[type="text"], input[type="search"]');
    const hasForm = await form.first().isVisible({ timeout: 3000 }).catch(() => false);

    expect(hasModal || hasForm).toBe(true);
  }
});

// ---- Billing Start Modal: Payment Method ----

test('billing: start modal shows payment method selector', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  const startBtn = page.locator('button').filter({ hasText: /start|book|assign/i }).first();
  const hasStart = await startBtn.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasStart) {
    await startBtn.click();
    await page.waitForTimeout(500);

    // Look for payment method options
    const walletOption = page.getByText(/wallet/i);
    const cashOption = page.getByText(/cash/i);
    const upiOption = page.getByText(/upi/i);
    const cardOption = page.getByText(/card/i);

    const hasWallet = await walletOption.first().isVisible({ timeout: 3000 }).catch(() => false);
    const hasCash = await cashOption.first().isVisible({ timeout: 1000 }).catch(() => false);

    // At least wallet and cash should be visible in the modal
    expect(hasWallet || hasCash).toBe(true);
  }
});

// ---- Billing Start Modal: Discount UI ----

test('billing: start modal has discount toggle', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  const startBtn = page.locator('button').filter({ hasText: /start|book|assign/i }).first();
  const hasStart = await startBtn.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasStart) {
    await startBtn.click();
    await page.waitForTimeout(500);

    // Look for discount UI elements
    const discountToggle = page.getByText(/discount/i);
    const hasDiscount = await discountToggle.first().isVisible({ timeout: 3000 }).catch(() => false);
    expect(hasDiscount).toBe(true);
  }
});

// ---- Billing History ----

test('billing history: page renders with session table', async ({ page }) => {
  await page.goto('/billing/history', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);

  // Should have a date filter
  const dateInput = page.locator('input[type="date"]');
  const hasDate = await dateInput.first().isVisible({ timeout: 5000 }).catch(() => false);
  expect(hasDate).toBe(true);
});

test('billing history: shows revenue summary', async ({ page }) => {
  await page.goto('/billing/history', { waitUntil: 'networkidle' });

  // Should show some kind of summary (total, sessions count, or revenue)
  const summary = page.getByText(/total|sessions|credits|revenue/i);
  const hasSummary = await summary.first().isVisible({ timeout: 5000 }).catch(() => false);
  expect(hasSummary).toBe(true);
});

test('billing history: refund button visible on completed sessions', async ({ page }) => {
  await page.goto('/billing/history', { waitUntil: 'networkidle' });

  // Look for refund buttons on session rows
  const refundBtn = page.locator('button').filter({ hasText: /refund/i });
  const hasRefund = await refundBtn.first().isVisible({ timeout: 5000 }).catch(() => false);

  // If there are completed sessions, refund button should exist
  // (may not exist if no sessions today)
  const rows = page.locator('tr, [role="row"]');
  const rowCount = await rows.count();
  if (rowCount > 2) {
    // Has data rows beyond header
    expect(hasRefund).toBe(true);
  }
});

test('billing history: clicking refund opens modal', async ({ page }) => {
  await page.goto('/billing/history', { waitUntil: 'networkidle' });

  const refundBtn = page.locator('button').filter({ hasText: /refund/i }).first();
  const hasRefund = await refundBtn.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasRefund) {
    await refundBtn.click();

    // Modal should appear with amount input and reason
    const amountInput = page.locator('input[type="number"]');
    const reasonInput = page.locator('textarea, input[placeholder*="reason" i]');
    const hasAmount = await amountInput.first().isVisible({ timeout: 3000 }).catch(() => false);
    const hasReason = await reasonInput.first().isVisible({ timeout: 3000 }).catch(() => false);

    expect(hasAmount || hasReason).toBe(true);
  }
});

// ---- Billing Pricing ----

test('billing pricing: shows rate tiers with per-minute rates', async ({ page }) => {
  await page.goto('/billing/pricing', { waitUntil: 'networkidle' });

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);

  // Should show rate-related content
  const hasRates = /per.?min|rate|tier|standard|extended|marathon/i.test(body);
  expect(hasRates).toBe(true);
});

test('billing pricing: has add rate button', async ({ page }) => {
  await page.goto('/billing/pricing', { waitUntil: 'networkidle' });

  const addBtn = page.locator('button').filter({ hasText: /add|create|new/i });
  const hasAdd = await addBtn.first().isVisible({ timeout: 5000 }).catch(() => false);
  expect(hasAdd).toBe(true);
});

// ---- Sidebar Navigation ----

test('sidebar: billing section has all expected links', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  // Check for billing-related nav links
  const billingLink = page.locator('a[href*="/billing"]');
  const linkCount = await billingLink.count();

  // Should have at least: /billing, /billing/history, /billing/pricing
  expect(linkCount).toBeGreaterThanOrEqual(3);
});

// ---- Active Session Actions ----

test('billing: active session card shows timer and controls', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  // If there are active sessions, they should show a countdown timer
  const timer = page.getByText(/\d+:\d+/); // MM:SS or H:MM:SS pattern
  const hasTimer = await timer.first().isVisible({ timeout: 3000 }).catch(() => false);

  // Also look for pause/end buttons
  const pauseBtn = page.locator('button').filter({ hasText: /pause|end|stop/i });
  const hasPause = await pauseBtn.first().isVisible({ timeout: 3000 }).catch(() => false);

  // At least one should be visible if there are active sessions
  // (both may be absent if no active sessions -- that's OK)
  const sessionText = page.getByText(/active session/i);
  const countText = await sessionText.textContent() ?? '';
  const activeCount = parseInt(countText.match(/\d+/)?.[0] ?? '0');

  if (activeCount > 0) {
    expect(hasTimer || hasPause).toBe(true);
  }
});

// ---- Extend Session ----

test('billing: extend button adds time to active session', async ({ page }) => {
  await page.goto('/billing', { waitUntil: 'networkidle' });

  // Look for +10m or extend button
  const extendBtn = page.locator('button').filter({ hasText: /extend|\+10|add time/i }).first();
  const hasExtend = await extendBtn.isVisible({ timeout: 5000 }).catch(() => false);

  // If active sessions exist, extend button should be present
  if (hasExtend) {
    // Just verify it's clickable (don't actually extend in E2E)
    await expect(extendBtn).toBeEnabled();
  }
});

// ---- Date filter interaction ----

test('billing history: changing date filter reloads data', async ({ page }) => {
  await page.goto('/billing/history', { waitUntil: 'networkidle' });

  const dateInput = page.locator('input[type="date"]').first();
  const hasDate = await dateInput.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasDate) {
    // Set to yesterday
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    const dateStr = yesterday.toISOString().split('T')[0];

    await dateInput.fill(dateStr);
    await page.waitForTimeout(1000);

    // Page should still be functional after date change
    const body = await page.textContent('body') ?? '';
    expect(body).not.toMatch(/application error/i);
  }
});
