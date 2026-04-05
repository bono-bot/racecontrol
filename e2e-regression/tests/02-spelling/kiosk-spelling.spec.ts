// ═══════════════════════════════════════════════════════════════
// Spelling Check — Kiosk pages
// ═══════════════════════════════════════════════════════════════

import { test } from '@playwright/test';
import { loginKioskStaff } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { KIOSK_PAGES, KIOSK_STAFF_PAGES } from '../../fixtures/test-data';

test.describe('02 — Kiosk Spelling Check', () => {
  test('Check all kiosk pages for spelling issues', async ({ page }) => {
    const allSuspicious: Array<{ page: string; words: string[] }> = [];

    // Public pages
    for (const pg of KIOSK_PAGES) {
      await page.goto(pg.path, { waitUntil: 'load' });
      await page.waitForTimeout(2000);

      const texts: string[] = await page.evaluate(() => {
        const t: string[] = [];
        const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        let node;
        while ((node = walker.nextNode())) {
          const text = node.textContent?.trim();
          if (text && text.length > 2) t.push(text);
        }
        return t;
      });

      // Check for suspicious patterns
      const words = texts.join(' ').split(/\s+/).filter(w => {
        const clean = w.replace(/[^a-zA-Z]/g, '').toLowerCase();
        return clean.length >= 4 && /(.)\1{2,}/.test(clean);
      });

      if (words.length > 0) {
        allSuspicious.push({ page: pg.path, words: [...new Set(words)] });
        await screenshot(page, `02-kiosk-spelling-${pg.name}`);
      }
    }

    // Staff pages
    await loginKioskStaff(page);
    for (const pg of KIOSK_STAFF_PAGES) {
      await page.goto(pg.path, { waitUntil: 'load' });
      await page.waitForTimeout(2000);

      const texts: string[] = await page.evaluate(() => {
        const t: string[] = [];
        const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        let node;
        while ((node = walker.nextNode())) {
          const text = node.textContent?.trim();
          if (text && text.length > 2) t.push(text);
        }
        return t;
      });

      const words = texts.join(' ').split(/\s+/).filter(w => {
        const clean = w.replace(/[^a-zA-Z]/g, '').toLowerCase();
        return clean.length >= 4 && /(.)\1{2,}/.test(clean);
      });

      if (words.length > 0) {
        allSuspicious.push({ page: pg.path, words: [...new Set(words)] });
        await screenshot(page, `02-kiosk-spelling-staff-${pg.name}`);
      }
    }

    console.log('\n═══ KIOSK SPELLING CHECK ═══');
    if (allSuspicious.length === 0) {
      console.log('No suspicious words found on kiosk pages.');
    } else {
      for (const entry of allSuspicious) {
        console.log(`  ${entry.page}: ${entry.words.join(', ')}`);
      }
    }
    console.log('═══════════════════════════\n');
  });
});
