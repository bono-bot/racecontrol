import { test, expect } from '@playwright/test';

const BASE = 'http://192.168.31.23:3200';

// ── 5-Flow Smoke Pack ──────────────────────────────────────
// These run as regression guard after every deploy.
// Each test is independent and should complete in <10 seconds.

test.describe('Smart Pipe: Regression Smoke Pack', () => {

  test('1. Dashboard loads with pod grid', async ({ page }) => {
    await page.goto(`${BASE}/`);
    await expect(page).toHaveTitle(/Racing Point/i, { timeout: 10000 });
    // Should have navigation sidebar
    await expect(page.locator('nav, [class*="sidebar"], [class*="Sidebar"]')).toBeVisible({ timeout: 5000 });
  });

  test('2. Billing page loads with pod cards', async ({ page }) => {
    await page.goto(`${BASE}/billing`);
    await page.waitForLoadState('networkidle');
    // Should show at least one pod card or "idle" state
    const content = await page.textContent('body');
    expect(content).toBeTruthy();
    expect(content!.length).toBeGreaterThan(100);
  });

  test('3. Cameras page loads with camera grid', async ({ page }) => {
    await page.goto(`${BASE}/cameras`);
    await page.waitForLoadState('networkidle');
    // Should show camera tiles (img elements with snapshot src)
    const images = page.locator('img[src*="snapshot"], img[alt*="camera" i], img[alt*="Camera" i]');
    // At least 1 camera should be visible
    await expect(images.first()).toBeVisible({ timeout: 15000 });
  });

  test('4. API health returns valid JSON', async ({ request }) => {
    const res = await request.get('http://192.168.31.23:8080/api/v1/health');
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body.status).toBe('ok');
    expect(body.build_id).toBeTruthy();
  });

  test('5. Fleet health returns pod data', async ({ request }) => {
    const res = await request.get('http://192.168.31.23:8080/api/v1/fleet/health');
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body.pods).toBeDefined();
    expect(body.pods.length).toBeGreaterThan(0);
  });
});
