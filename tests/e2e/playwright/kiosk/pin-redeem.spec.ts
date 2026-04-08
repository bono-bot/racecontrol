import { test, expect } from '@playwright/test';

// PIN Redeem tests — SKIPPED
// The "Redeem PIN" button and modal do not exist on the current kiosk landing page.
// The redeem-pin API endpoint exists (api.ts) but the UI component was never built.
// These tests will be re-enabled when the PIN redeem feature is implemented.

test.describe('PIN Redeem — Entry state', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows 6 empty PIN boxes and character grid', async ({ page }) => {
    // Placeholder — will be implemented when PIN redeem UI is built
  });

  test('typing fills PIN boxes and enables submit at 6 chars', async ({ page }) => {
    // Placeholder
  });

  test('backspace removes last character', async ({ page }) => {
    // Placeholder
  });

  test('clear button resets all characters', async ({ page }) => {
    // Placeholder
  });

  test('close button dismisses the modal', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Validating state', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows spinner while validating', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Success state', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows pod number, experience, tier, minutes, and driver name', async ({ page }) => {
    // Placeholder
  });

  test('success screen auto-closes after timeout', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Error state (invalid PIN)', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows error message and remaining attempts', async ({ page }) => {
    // Placeholder
  });

  test('Try Again returns to entry state with empty PIN', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Pending debit state', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows yellow clock icon and "Booking in Progress" message', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Infrastructure error', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows error without attempts counter (no lockout penalty)', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Lockout state', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows lockout screen with countdown timer', async ({ page }) => {
    // Placeholder
  });

  test('lockout countdown decrements', async ({ page }) => {
    // Placeholder
  });
});

test.describe('PIN Redeem — Network error', () => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');

  test('shows generic network error on fetch failure', async ({ page }) => {
    // Placeholder
  });
});

test('PIN flow produces no uncaught JS errors on happy path', async ({ page }) => {
  test.skip(true, 'PIN redeem UI not implemented on current kiosk landing page');
});
