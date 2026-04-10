// ═══════════════════════════════════════════════════════════════
// Auth helpers — login for POS (web dashboard) and Kiosk (staff)
// ═══════════════════════════════════════════════════════════════

import { Page } from '@playwright/test';
import { API_BASE, STAFF_PIN, ADMIN_PIN } from './test-data';
import { RCApiClient } from './api-client';

// Delegate to RCApiClient which handles file-persisted token caching
const sharedApi = new RCApiClient();

// Get staff JWT from API (uses shared file-persisted token)
export async function getStaffToken(pin: string = STAFF_PIN): Promise<string> {
  return sharedApi.login(pin);
}

// Login to POS web dashboard (inject JWT into localStorage)
export async function loginPOS(page: Page, pin: string = STAFF_PIN): Promise<void> {
  const token = await getStaffToken(pin);
  await page.goto('/login', { waitUntil: 'load' });
  await page.evaluate((t) => {
    localStorage.setItem('rp_staff_jwt', t);
  }, token);
  await page.goto('/', { waitUntil: 'load' });
  await page.waitForTimeout(2000);
}

// Login to kiosk staff terminal
export async function loginKioskStaff(page: Page, pin: string = STAFF_PIN): Promise<void> {
  // Navigate to staff page
  await page.goto('/kiosk/staff', { waitUntil: 'load' });
  await page.waitForTimeout(1000);

  // Try to find and fill PIN input
  const pinInput = page.locator('input[type="password"], input[type="tel"], input#pin, input[name="pin"]').first();
  const hasPinInput = await pinInput.isVisible({ timeout: 5000 }).catch(() => false);

  if (hasPinInput) {
    await pinInput.fill(pin);
    // Submit
    const submitBtn = page.locator('button[type="submit"], button:has-text("Login"), button:has-text("Enter")').first();
    await submitBtn.click();
    await page.waitForTimeout(2000);
  } else {
    // May already be logged in or different auth flow — inject JWT
    const token = await getStaffToken(pin);
    await page.evaluate((t) => {
      localStorage.setItem('kiosk_staff_jwt', t);
      localStorage.setItem('rp_staff_jwt', t);
      // Also set cookie — middleware checks cookie, not localStorage
      document.cookie = `kiosk_staff_jwt=${t}; path=/; max-age=1800; SameSite=Strict`;
    }, token);
    await page.goto('/kiosk/staff', { waitUntil: 'load' });
    await page.waitForTimeout(2000);
  }
}

// Wait for app to hydrate (sidebar visible, data loaded)
export async function waitForApp(page: Page): Promise<void> {
  await page.waitForSelector('aside, nav, .sidebar, [data-testid="sidebar"]', { timeout: 10000 }).catch(() => null);
  await page.waitForTimeout(2000);
}

// Get admin JWT from API (validates with ADMIN_PIN)
export async function getAdminToken(baseUrl: string = API_BASE): Promise<string> {
  const pin = process.env.ADMIN_PIN || ADMIN_PIN;
  if (!pin) throw new Error('ADMIN_PIN env var or test-data constant required');

  // Try validate-pin first (staff endpoint works for admin too)
  const resp = await fetch(`${baseUrl}/staff/validate-pin`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pin }),
  });

  if (resp.ok) {
    const data = await resp.json();
    if (data.token) return data.token;
  }

  // Fallback to admin-login endpoint
  const resp2 = await fetch(`${baseUrl}/auth/admin-login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pin }),
  });

  if (resp2.ok) {
    const data = await resp2.json();
    if (data.token) return data.token;
  }

  throw new Error(`Admin login failed on both endpoints: validate-pin=${resp.status}, admin-login=${resp2.status}`);
}

// Clear cached token (for testing wrong PIN scenarios)
export function clearCachedToken(): void {
  cachedStaffToken = null;
}
