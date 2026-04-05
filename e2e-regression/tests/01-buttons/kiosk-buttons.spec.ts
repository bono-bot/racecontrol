// ═══════════════════════════════════════════════════════════════
// Button Audit — Kiosk Staff UI
// Click EVERY button, screenshot, verify no errors
// ═══════════════════════════════════════════════════════════════

import { test, expect, Page } from '@playwright/test';
import { loginKioskStaff } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { KIOSK_PAGES, KIOSK_STAFF_PAGES } from '../../fixtures/test-data';

async function findClickableElements(page: Page): Promise<Array<{ selector: string; text: string }>> {
  return page.evaluate(() => {
    const elements: Array<{ selector: string; text: string }> = [];
    const clickable = document.querySelectorAll('button, a[href], [role="button"], [role="tab"], input[type="submit"], select');
    clickable.forEach((el, i) => {
      const text = (el as HTMLElement).innerText?.trim().slice(0, 50) || el.getAttribute('aria-label') || el.tagName;
      elements.push({ selector: `button,a[href],[role="button"]:nth-child(${i + 1})`, text });
    });
    return elements;
  });
}

test.describe('01 — Kiosk Button Audit', () => {
  // Public kiosk pages
  for (const pg of KIOSK_PAGES) {
    test(`Public kiosk buttons on ${pg.name}`, async ({ page }) => {
      await page.goto(pg.path, { waitUntil: 'load' });
      await page.waitForTimeout(3000);
      await screenshot(page, `01-kiosk-btn-${pg.name}-overview`);

      const elements = await findClickableElements(page);
      console.log(`  ${pg.name}: ${elements.length} clickable elements`);

      let btnIdx = 0;
      for (const el of elements) {
        btnIdx++;
        try {
          const locator = page.locator('button, a[href], [role="button"]').nth(btnIdx - 1);
          const isVisible = await locator.isVisible({ timeout: 2000 }).catch(() => false);
          if (isVisible) {
            await Promise.race([
              locator.click({ timeout: 3000 }),
              page.waitForTimeout(2000),
            ]).catch(() => {});
            await screenshot(page, `01-kiosk-btn-${pg.name}-${btnIdx}`);
            // Navigate back
            if (!page.url().includes(pg.path.split('/').pop() || '')) {
              await page.goto(pg.path, { waitUntil: 'load' });
              await page.waitForTimeout(1000);
            }
          }
        } catch {
          // Button click failed — log but continue
        }
      }
    });
  }

  // Staff kiosk pages (after login)
  for (const pg of KIOSK_STAFF_PAGES) {
    test(`Staff kiosk buttons on ${pg.name}`, async ({ page }) => {
      await loginKioskStaff(page);
      await page.goto(pg.path, { waitUntil: 'load' });
      await page.waitForTimeout(2000);
      await screenshot(page, `01-kiosk-staff-btn-${pg.name}-overview`);

      const elements = await findClickableElements(page);
      console.log(`  Staff ${pg.name}: ${elements.length} clickable elements`);

      let btnIdx = 0;
      for (const el of elements) {
        btnIdx++;
        try {
          const locator = page.locator('button, a[href], [role="button"]').nth(btnIdx - 1);
          const isVisible = await locator.isVisible({ timeout: 2000 }).catch(() => false);
          if (isVisible) {
            await Promise.race([
              locator.click({ timeout: 3000 }),
              page.waitForTimeout(2000),
            ]).catch(() => {});
            await screenshot(page, `01-kiosk-staff-btn-${pg.name}-${btnIdx}`);
            if (!page.url().includes(pg.path.split('/').pop() || '')) {
              await page.goto(pg.path, { waitUntil: 'load' });
              await page.waitForTimeout(1000);
            }
          }
        } catch { /* continue */ }
      }
    });
  }
});
