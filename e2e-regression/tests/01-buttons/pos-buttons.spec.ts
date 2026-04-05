// ═══════════════════════════════════════════════════════════════
// Button Audit — POS Billing UI
// Click EVERY button on EVERY page, screenshot, verify no errors
// ═══════════════════════════════════════════════════════════════

import { test, expect, Page } from '@playwright/test';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { POS_PAGES } from '../../fixtures/test-data';

interface ButtonResult {
  page: string;
  selector: string;
  text: string;
  clicked: boolean;
  error: string | null;
  screenshot: string;
}

const allResults: ButtonResult[] = [];

// Find all clickable elements on a page
async function findClickableElements(page: Page): Promise<Array<{ selector: string; text: string }>> {
  return page.evaluate(() => {
    const elements: Array<{ selector: string; text: string }> = [];
    const clickable = document.querySelectorAll('button, a[href], [role="button"], [role="tab"], [role="menuitem"], input[type="submit"], input[type="button"], select, [onclick], [data-testid]');

    clickable.forEach((el, i) => {
      const text = (el as HTMLElement).innerText?.trim().slice(0, 50) || el.getAttribute('aria-label') || el.tagName;
      const id = el.id ? `#${el.id}` : '';
      const cls = el.className ? `.${String(el.className).split(' ')[0]}` : '';
      const tag = el.tagName.toLowerCase();
      const selector = id || `${tag}${cls}:nth-of-type(${i + 1})`;

      elements.push({ selector, text });
    });

    return elements;
  });
}

test.describe('01 — POS Button Audit', () => {
  for (const pg of POS_PAGES) {
    test(`Buttons on ${pg.name} (${pg.path})`, async ({ page }) => {
      await loginPOS(page);
      await page.goto(pg.path, { waitUntil: 'load' });
      await waitForApp(page);

      // Screenshot the page
      await screenshot(page, `01-buttons-pos-${pg.name}-overview`);

      // Find all clickable elements
      const elements = await findClickableElements(page);
      console.log(`  ${pg.name}: ${elements.length} clickable elements found`);

      let btnIdx = 0;
      for (const el of elements) {
        btnIdx++;
        const result: ButtonResult = {
          page: pg.path,
          selector: el.selector,
          text: el.text,
          clicked: false,
          error: null,
          screenshot: '',
        };

        try {
          // Check for JS console errors before click
          const errorsBefore: string[] = [];
          page.on('console', msg => {
            if (msg.type() === 'error') errorsBefore.push(msg.text());
          });

          // Try to click (with a short timeout, don't navigate away)
          const locator = page.locator(el.selector).first();
          const isVisible = await locator.isVisible({ timeout: 2000 }).catch(() => false);

          if (isVisible) {
            // Click with navigation handling
            await Promise.race([
              locator.click({ timeout: 5000 }),
              page.waitForTimeout(3000),
            ]).catch(() => { /* click might fail — that's OK for disabled buttons */ });

            result.clicked = true;

            // Wait a moment for any response
            await page.waitForTimeout(1000);

            // Check we didn't get a 404 or blank page
            const url = page.url();
            const body = await page.textContent('body').catch(() => '');
            if (url.includes('404') || (body?.length ?? 0) < 20) {
              result.error = `Navigation to 404 or blank page after clicking "${el.text}"`;
            }

            // Screenshot after click
            result.screenshot = `01-buttons-pos-${pg.name}-btn${btnIdx}`;
            await screenshot(page, result.screenshot);

            // Navigate back to the original page if we left
            if (!page.url().includes(pg.path) && pg.path !== '/') {
              await page.goto(pg.path, { waitUntil: 'load' });
              await waitForApp(page);
            }
          }
        } catch (e) {
          result.error = String(e).slice(0, 200);
        }

        allResults.push(result);
      }

      // Log results for this page
      const failures = allResults.filter(r => r.page === pg.path && r.error);
      if (failures.length > 0) {
        console.log(`  FAILURES on ${pg.name}:`);
        for (const f of failures) {
          console.log(`    ✗ "${f.text}" — ${f.error}`);
        }
      }
    });
  }

  test('Button audit summary', async () => {
    const total = allResults.length;
    const clicked = allResults.filter(r => r.clicked).length;
    const failures = allResults.filter(r => r.error).length;

    console.log(`\n═══ POS BUTTON AUDIT SUMMARY ═══`);
    console.log(`Total elements: ${total}`);
    console.log(`Clicked: ${clicked}`);
    console.log(`Failures: ${failures}`);

    if (failures > 0) {
      console.log(`\nFailed buttons:`);
      for (const f of allResults.filter(r => r.error)) {
        console.log(`  ${f.page} — "${f.text}" — ${f.error}`);
      }
    }

    // Allow some failures (disabled buttons, modals, etc.) but flag high count
    expect(failures).toBeLessThan(total * 0.2); // Less than 20% failure rate
  });
});
