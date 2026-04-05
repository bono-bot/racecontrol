// ═══════════════════════════════════════════════════════════════
// Spelling Check — crawl all POS pages, extract text, check
// ═══════════════════════════════════════════════════════════════

import { test, expect, Page } from '@playwright/test';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { POS_PAGES } from '../../fixtures/test-data';

// Known domain terms that are NOT misspellings
const DOMAIN_TERMS = new Set([
  'racingpoint', 'racecontrol', 'assetto', 'corsa', 'iracing', 'forza',
  'sim', 'sims', 'motorsport', 'kiosk', 'hud', 'ffb', 'acs', 'udp',
  'webhook', 'websocket', 'ws', 'api', 'jwt', 'otp', 'upi', 'pos',
  'monza', 'spa', 'silverstone', 'nordschleife', 'mugello', 'suzuka',
  'telemetry', 'leaderboard', 'leaderboards', 'hotlap', 'hotlaps',
  'ferrari', 'porsche', 'lamborghini', 'mclaren', 'bmw',
  'paise', 'rupee', 'rupees', 'inr', 'topup', 'refund',
  'ui', 'url', 'html', 'css', 'tsx', 'localhost',
  'signin', 'signup', 'logout', 'navbar', 'sidebar',
  'conspit', 'conspitlink', 'openffboard',
  'evo', 'wrc', 'lmu', 'lms',
  'chavan', 'vishal', 'uday', 'singh',
]);

// Common abbreviations and short words to skip
const SKIP_PATTERNS = /^[A-Z0-9]{1,4}$|^[a-z]{1,2}$|^\d+$/;

async function extractPageText(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const texts: string[] = [];
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
      null,
    );
    let node;
    while ((node = walker.nextNode())) {
      const text = node.textContent?.trim();
      if (text && text.length > 2) {
        texts.push(text);
      }
    }
    return texts;
  });
}

// Simple heuristic: check for common misspelling patterns
function findSuspiciousWords(texts: string[]): string[] {
  const suspicious: string[] = [];
  const allWords = texts.join(' ').split(/\s+/);

  for (const word of allWords) {
    const clean = word.replace(/[^a-zA-Z]/g, '').toLowerCase();
    if (clean.length < 4) continue;
    if (DOMAIN_TERMS.has(clean)) continue;
    if (SKIP_PATTERNS.test(clean)) continue;

    // Check for repeated characters (typo indicator)
    if (/(.)\1{2,}/.test(clean)) {
      suspicious.push(word);
    }

    // Check for uncommon character sequences
    if (/[bcdfghjklmnpqrstvwxyz]{5,}/.test(clean)) {
      suspicious.push(word);
    }
  }

  return [...new Set(suspicious)];
}

test.describe('02 — POS Spelling Check', () => {
  test('Check all POS pages for spelling issues', async ({ page }) => {
    await loginPOS(page);

    const allSuspicious: Array<{ page: string; words: string[] }> = [];

    for (const pg of POS_PAGES) {
      await page.goto(pg.path, { waitUntil: 'load' });
      await waitForApp(page);

      const texts = await extractPageText(page);
      const suspicious = findSuspiciousWords(texts);

      if (suspicious.length > 0) {
        allSuspicious.push({ page: pg.path, words: suspicious });
        await screenshot(page, `02-spelling-${pg.name}`);
      }
    }

    console.log('\n═══ SPELLING CHECK REPORT ═══');
    if (allSuspicious.length === 0) {
      console.log('No suspicious words found.');
    } else {
      for (const entry of allSuspicious) {
        console.log(`  ${entry.page}: ${entry.words.join(', ')}`);
      }
    }
    console.log('═════════════════════════════\n');

    // Don't fail the test — just report
    // Suspicious words may be false positives
  });
});
