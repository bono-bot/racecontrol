// ═══════════════════════════════════════════════════════════════
// Chaos Tests — Kiosk
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { loginKioskStaff } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';

test.describe('08 — Kiosk Chaos Tests', () => {
  test('Lock screen refresh — stays on lock screen', async ({ page }) => {
    await page.goto('/kiosk/', { waitUntil: 'load' });
    await page.waitForTimeout(2000);
    await page.reload({ waitUntil: 'load' });
    await page.waitForTimeout(2000);
    await screenshot(page, '08-kiosk-chaos-refresh');

    const body = await page.textContent('body');
    expect(body?.length).toBeGreaterThan(50);
  });

  test('Rapid kiosk page switching — no errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') errors.push(msg.text());
    });

    const pages = ['/kiosk/', '/kiosk/spectator', '/kiosk/register', '/kiosk/'];
    for (const pg of pages) {
      await page.goto(pg, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(300);
    }

    await page.waitForTimeout(2000);
    await screenshot(page, '08-kiosk-chaos-rapid');

    const fatalErrors = errors.filter(e =>
      e.includes('Uncaught') || e.includes('TypeError') || e.includes('ReferenceError')
    );
    expect(fatalErrors.length).toBe(0);
  });

  test('Staff kiosk — rapid fleet page refresh', async ({ page }) => {
    await loginKioskStaff(page);
    await page.goto('/kiosk/fleet', { waitUntil: 'load' });
    await page.waitForTimeout(1000);

    // Rapid refresh 5 times
    for (let i = 0; i < 5; i++) {
      await page.reload({ waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(500);
    }

    await screenshot(page, '08-kiosk-chaos-fleet-refresh');
    expect(page.url()).toContain('fleet');
  });
});
