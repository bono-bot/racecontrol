import { test, expect } from '@playwright/test';
import { loginKioskStaff } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { KIOSK_PAGES, KIOSK_STAFF_PAGES } from '../../fixtures/test-data';

test.describe('00 — Kiosk Smoke Tests', () => {
  test('Kiosk lock screen renders', async ({ page }) => {
    await page.goto('/kiosk/', { waitUntil: 'load' });
    await page.waitForTimeout(3000);
    await screenshot(page, '00-kiosk-lock-screen');

    // Lock screen should have Racing Point branding or content
    const body = await page.textContent('body');
    expect(body?.length).toBeGreaterThan(50);
  });

  test('Kiosk registration page loads', async ({ page }) => {
    await page.goto('/kiosk/register', { waitUntil: 'load' });
    await page.waitForTimeout(2000);
    await screenshot(page, '00-kiosk-register');
    // May redirect to lock screen — either is valid
    const body = await page.textContent('body');
    expect(body?.length).toBeGreaterThan(50);
  });

  test('Kiosk spectator page loads', async ({ page }) => {
    await page.goto('/kiosk/spectator', { waitUntil: 'load' });
    await page.waitForTimeout(2000);
    await screenshot(page, '00-kiosk-spectator');
  });

  test('Kiosk staff login page loads', async ({ page }) => {
    await page.goto('/kiosk/staff', { waitUntil: 'load' });
    await page.waitForTimeout(2000);
    await screenshot(page, '00-kiosk-staff-login');
    // Should have a PIN input or login form
    const body = await page.textContent('body');
    expect(body?.length).toBeGreaterThan(50);
  });

  test('Kiosk staff pages load after login', async ({ page }) => {
    await loginKioskStaff(page);

    for (const pg of KIOSK_STAFF_PAGES) {
      await page.goto(pg.path, { waitUntil: 'load' });
      await page.waitForTimeout(2000);
      await screenshot(page, `00-kiosk-staff-${pg.name.toLowerCase()}`);
    }
  });
});
