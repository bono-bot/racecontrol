// ═══════════════════════════════════════════════════════════════
// Auth helpers — login for POS (web dashboard) and Kiosk (staff)
// ═══════════════════════════════════════════════════════════════

import { Page } from '@playwright/test';
import { API_BASE, STAFF_PIN } from './test-data';

let cachedStaffToken: string | null = null;

// Get staff JWT from API
export async function getStaffToken(pin: string = STAFF_PIN): Promise<string> {
  if (cachedStaffToken) return cachedStaffToken;

  // Retry with backoff for 429 rate limiting
  for (let attempt = 0; attempt < 5; attempt++) {
    if (attempt > 0) await new Promise(r => setTimeout(r, Math.pow(2, attempt) * 1000));

    const resp = await fetch(`${API_BASE}/staff/validate-pin`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ pin }),
    });

    if (resp.status === 429) {
      console.log(`  Auth rate limited (429), retry ${attempt + 1}/5...`);
      continue;
    }

    if (resp.ok) {
      const data = await resp.json();
      cachedStaffToken = data.token;
      return data.token;
    }

    // Fallback to admin-login
    const resp2 = await fetch(`${API_BASE}/auth/admin-login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ pin }),
    });
    if (resp2.status === 429) continue;
    if (!resp2.ok) throw new Error(`Staff login failed: ${resp2.status}`);
    const data2 = await resp2.json();
    cachedStaffToken = data2.token;
    return data2.token;
  }
  throw new Error('Staff login failed after 5 retries (rate limited)');
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

// Clear cached token (for testing wrong PIN scenarios)
export function clearCachedToken(): void {
  cachedStaffToken = null;
}
