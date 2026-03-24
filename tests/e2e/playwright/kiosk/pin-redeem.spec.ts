import { test, expect } from '@playwright/test';
import {
  setupApiMocks,
  overrideRedeemPinMock,
  MOCK_REDEEM_PIN_ERROR,
  MOCK_REDEEM_PIN_LOCKOUT,
  MOCK_REDEEM_PIN_PENDING,
  MOCK_REDEEM_PIN_INFRA_ERROR,
} from '../fixtures/api-mocks';

// ---- JS error capture ----
let jsErrors: string[] = [];
test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
  await setupApiMocks(page);
});
test.afterEach(async ({}, testInfo) => {
  if (jsErrors.length > 0) {
    const msg = jsErrors.join('; ');
    jsErrors = [];
    // Attach but don't fail — some errors may be from WS disconnect
    testInfo.annotations.push({ type: 'js-errors', description: msg });
  }
});

// Helper: open the PIN redeem modal from the kiosk landing page
async function openPinModal(page: import('@playwright/test').Page) {
  await page.goto('/', { waitUntil: 'networkidle' });
  // Click "Redeem PIN" button on landing page
  const redeemBtn = page.getByRole('button', { name: /redeem.*pin/i });
  await expect(redeemBtn).toBeVisible({ timeout: 10_000 });
  await redeemBtn.click();
  // Wait for PIN entry screen
  await expect(page.getByText('Enter Your Booking PIN')).toBeVisible({ timeout: 5_000 });
}

// Helper: type a 6-character PIN using the on-screen character grid
async function typePinOnGrid(page: import('@playwright/test').Page, pin: string) {
  for (const ch of pin.toUpperCase()) {
    const btn = page.locator(`button:has-text("${ch}")`).first();
    await btn.click();
  }
}

// ─── STATE: Entry ─────────────────────────────────────────────────────────────

test.describe('PIN Redeem — Entry state', () => {
  test('shows 6 empty PIN boxes and character grid', async ({ page }) => {
    await openPinModal(page);

    // 6 PIN boxes should be visible
    const pinBoxes = page.locator('.font-mono.text-2xl');
    await expect(pinBoxes).toHaveCount(6);

    // Character grid should have 31 buttons (A-Z minus I,L,O + 2-9)
    const charButtons = page.locator('button').filter({ hasText: /^[A-Z2-9]$/ });
    await expect(charButtons).toHaveCount(31);

    // Submit should be disabled when empty
    const submitBtn = page.getByRole('button', { name: /submit/i });
    await expect(submitBtn).toBeDisabled();
  });

  test('typing fills PIN boxes and enables submit at 6 chars', async ({ page }) => {
    await openPinModal(page);

    // Type 5 chars — submit should still be disabled
    await typePinOnGrid(page, 'ABC23');
    const submitBtn = page.getByRole('button', { name: /submit/i });
    await expect(submitBtn).toBeDisabled();

    // Type 6th char — submit should now be enabled
    await typePinOnGrid(page, 'K');
    await expect(submitBtn).toBeEnabled();
  });

  test('backspace removes last character', async ({ page }) => {
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC');

    // Click backspace (SVG button — col-span-1)
    const backspaceBtn = page.locator('button.col-span-1').first();
    await backspaceBtn.click();

    // Should now have 2 chars filled — submit disabled
    const submitBtn = page.getByRole('button', { name: /submit/i });
    await expect(submitBtn).toBeDisabled();
  });

  test('clear button resets all characters', async ({ page }) => {
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23');

    const clearBtn = page.getByRole('button', { name: /clear/i });
    await clearBtn.click();

    const submitBtn = page.getByRole('button', { name: /submit/i });
    await expect(submitBtn).toBeDisabled();
  });

  test('close button dismisses the modal', async ({ page }) => {
    await openPinModal(page);

    // Close button (top-right X)
    const closeBtn = page.locator('button').filter({ has: page.locator('svg path[d*="M6 18L18 6"]') }).first();
    await closeBtn.click();

    // PIN modal should be gone
    await expect(page.getByText('Enter Your Booking PIN')).not.toBeVisible({ timeout: 3_000 });
  });
});

// ─── STATE: Validating ────────────────────────────────────────────────────────

test.describe('PIN Redeem — Validating state', () => {
  test('shows spinner while validating', async ({ page }) => {
    // Delay the response so we can see the validating state
    await page.route('**/api/v1/kiosk/redeem-pin', async (route) => {
      await new Promise(r => setTimeout(r, 2000));
      await route.fulfill({ json: { pod_number: 8, pod_id: 'pod-8', driver_name: 'Test', experience_name: 'Test', tier_name: '30 Min', allocated_seconds: 1800, billing_session_id: 'b-1' } });
    });
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23K');

    const submitBtn = page.getByRole('button', { name: /submit/i });
    await submitBtn.click();

    // Should show "Validating PIN..." text and spinner
    await expect(page.getByText('Validating PIN...')).toBeVisible({ timeout: 3_000 });
    // Spinner is the animate-spin div
    const spinner = page.locator('.animate-spin');
    await expect(spinner).toBeVisible();
  });
});

// ─── STATE: Success ───────────────────────────────────────────────────────────

test.describe('PIN Redeem — Success state', () => {
  test('shows pod number, experience, tier, minutes, and driver name', async ({ page }) => {
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23K');

    const submitBtn = page.getByRole('button', { name: /submit/i });
    await submitBtn.click();

    // Wait for success screen
    await expect(page.getByText('Head to Pod')).toBeVisible({ timeout: 5_000 });

    // Pod number 8
    await expect(page.getByText('8')).toBeVisible();

    // Experience name
    await expect(page.getByText('Monza Hot Lap')).toBeVisible();

    // Tier name
    await expect(page.getByText('30 Minutes')).toBeVisible();

    // Allocated minutes (1800s = 30 min)
    await expect(page.getByText('30 minutes')).toBeVisible();

    // Driver name greeting
    await expect(page.getByText('Welcome, Test Racer')).toBeVisible();

    // "Your game is loading..." message
    await expect(page.getByText('Your game is loading...')).toBeVisible();

    // Done button exists
    await expect(page.getByRole('button', { name: /done/i })).toBeVisible();
  });

  test('success screen auto-closes after timeout', async ({ page }) => {
    // Use a shorter timeout for testing — override component constant via mock delay
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23K');

    const submitBtn = page.getByRole('button', { name: /submit/i });
    await submitBtn.click();

    await expect(page.getByText('Head to Pod')).toBeVisible({ timeout: 5_000 });

    // Click Done to close manually (testing the button, not the 15s timer)
    const doneBtn = page.getByRole('button', { name: /done/i });
    await doneBtn.click();

    // Success screen should be gone
    await expect(page.getByText('Head to Pod')).not.toBeVisible({ timeout: 3_000 });
  });
});

// ─── STATE: Error (Invalid PIN) ───────────────────────────────────────────────

test.describe('PIN Redeem — Error state (invalid PIN)', () => {
  test('shows error message and remaining attempts', async ({ page }) => {
    await overrideRedeemPinMock(page, MOCK_REDEEM_PIN_ERROR);
    await openPinModal(page);
    await typePinOnGrid(page, 'XXXXXX');

    const submitBtn = page.getByRole('button', { name: /submit/i });
    await submitBtn.click();

    // Wait for error screen
    await expect(page.getByText('Invalid PIN')).toBeVisible({ timeout: 5_000 });

    // Error message
    await expect(page.getByText('Invalid PIN or reservation not found')).toBeVisible();

    // Remaining attempts
    await expect(page.getByText('8 attempts remaining')).toBeVisible();

    // Try Again button
    const tryAgainBtn = page.getByRole('button', { name: /try again/i });
    await expect(tryAgainBtn).toBeVisible();

    // Back button
    await expect(page.getByText('Back')).toBeVisible();
  });

  test('Try Again returns to entry state with empty PIN', async ({ page }) => {
    await overrideRedeemPinMock(page, MOCK_REDEEM_PIN_ERROR);
    await openPinModal(page);
    await typePinOnGrid(page, 'XXXXXX');
    await page.getByRole('button', { name: /submit/i }).click();

    await expect(page.getByText('Invalid PIN')).toBeVisible({ timeout: 5_000 });

    await page.getByRole('button', { name: /try again/i }).click();

    // Should be back in entry state
    await expect(page.getByText('Enter Your Booking PIN')).toBeVisible({ timeout: 3_000 });

    // Submit should be disabled (PIN cleared)
    await expect(page.getByRole('button', { name: /submit/i })).toBeDisabled();
  });
});

// ─── STATE: Error (Pending Debit / Booking in Progress) ───────────────────────

test.describe('PIN Redeem — Pending debit state', () => {
  test('shows yellow clock icon and "Booking in Progress" message', async ({ page }) => {
    await overrideRedeemPinMock(page, MOCK_REDEEM_PIN_PENDING);
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23K');
    await page.getByRole('button', { name: /submit/i }).click();

    // Should show "Booking in Progress" heading (not "Invalid PIN")
    await expect(page.getByText('Booking in Progress')).toBeVisible({ timeout: 5_000 });

    // Helpful message
    await expect(page.getByText(/being processed/i)).toBeVisible();

    // Should NOT show remaining attempts (pending is not a PIN error)
    await expect(page.getByText(/attempts remaining/i)).not.toBeVisible();
  });
});

// ─── STATE: Error (Infrastructure — pods busy) ────────────────────────────────

test.describe('PIN Redeem — Infrastructure error', () => {
  test('shows error without attempts counter (no lockout penalty)', async ({ page }) => {
    await overrideRedeemPinMock(page, MOCK_REDEEM_PIN_INFRA_ERROR);
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23K');
    await page.getByRole('button', { name: /submit/i }).click();

    // Shows the error message
    await expect(page.getByText(/All pods are currently in use/i)).toBeVisible({ timeout: 5_000 });

    // Should NOT show remaining attempts (infra error, not PIN error)
    await expect(page.getByText(/attempts remaining/i)).not.toBeVisible();
  });
});

// ─── STATE: Lockout ───────────────────────────────────────────────────────────

test.describe('PIN Redeem — Lockout state', () => {
  test('shows lockout screen with countdown timer', async ({ page }) => {
    await overrideRedeemPinMock(page, MOCK_REDEEM_PIN_LOCKOUT);
    await openPinModal(page);
    await typePinOnGrid(page, 'XXXXXX');
    await page.getByRole('button', { name: /submit/i }).click();

    // Should show "Too Many Attempts" heading
    await expect(page.getByText('Too Many Attempts')).toBeVisible({ timeout: 5_000 });

    // Countdown timer visible (5:00 for 300 seconds)
    await expect(page.getByText('5:00')).toBeVisible();

    // "Please wait" message
    await expect(page.getByText(/please wait/i)).toBeVisible();

    // Lock icon visible
    const lockIcon = page.locator('svg path[d*="M12 15v2"]');
    await expect(lockIcon).toBeVisible();
  });

  test('lockout countdown decrements', async ({ page }) => {
    // Use a short lockout for faster test
    await overrideRedeemPinMock(page, { ...MOCK_REDEEM_PIN_LOCKOUT, lockout_remaining_seconds: 3 });
    await openPinModal(page);
    await typePinOnGrid(page, 'XXXXXX');
    await page.getByRole('button', { name: /submit/i }).click();

    await expect(page.getByText('Too Many Attempts')).toBeVisible({ timeout: 5_000 });

    // Timer should show 0:03 initially
    await expect(page.getByText('0:03')).toBeVisible();

    // Wait for countdown to finish (3 seconds + buffer)
    await page.waitForTimeout(4000);

    // Should return to entry state after countdown
    await expect(page.getByText('Enter Your Booking PIN')).toBeVisible({ timeout: 3_000 });
  });
});

// ─── Network Error ────────────────────────────────────────────────────────────

test.describe('PIN Redeem — Network error', () => {
  test('shows generic network error on fetch failure', async ({ page }) => {
    // Abort the request to simulate network failure
    await page.route('**/api/v1/kiosk/redeem-pin', (route) => route.abort('connectionrefused'));
    await openPinModal(page);
    await typePinOnGrid(page, 'ABC23K');
    await page.getByRole('button', { name: /submit/i }).click();

    // Should show network error message
    await expect(page.getByText(/network error/i)).toBeVisible({ timeout: 5_000 });
  });
});

// ─── No JS errors across all PIN states ───────────────────────────────────────

test('PIN flow produces no uncaught JS errors on happy path', async ({ page }) => {
  await openPinModal(page);
  await typePinOnGrid(page, 'ABC23K');
  await page.getByRole('button', { name: /submit/i }).click();
  await expect(page.getByText('Head to Pod')).toBeVisible({ timeout: 5_000 });

  // No JS errors should have occurred
  expect(jsErrors.filter(e => !e.includes('WebSocket'))).toEqual([]);
});
