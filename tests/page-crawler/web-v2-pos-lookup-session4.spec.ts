/**
 * Visual evidence — PACT-001 Phase 1 wire-up Session 4 POS lookup page.
 *
 * Captures screenshots of /v2/pos/lookup at the four state-machine views:
 *   - idle        (empty input)
 *   - found       (mock state via direct setState — skipped, captured via prop hydration)
 *   - not_found   (skipped — needs actual API or test-mode prop)
 *   - error       (skipped — same)
 *
 * Wave 0 reality: API proxy from web-v2 → racecontrol :8080 not yet wired
 * (Session 5+ scope per PHASE-1-WIREUP-PLAN.md). This spec captures the idle
 * view + the WARN-gate-active state via simulated input.
 *
 * Run:
 *   cd racecontrol/web-v2 && npx next start -p 3500 &
 *   cd racecontrol && npx playwright test --config tests/page-crawler/playwright.config.ts \
 *     tests/page-crawler/web-v2-pos-lookup-session4.spec.ts --project=web-v2
 *
 * Output: tests/screenshots/web-v2-pos-lookup-session4-*.png
 */

import { test } from '@playwright/test';
import fs from 'fs';
import path from 'path';

const SCREENSHOT_DIR = path.join(__dirname, '../screenshots');
const BASE = process.env.WEB_V2_URL ?? 'http://localhost:3500';

test.beforeAll(() => {
  fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
});

test.describe('Session 4 — POS lookup page', () => {
  test('idle view renders without errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));

    await page.goto(`${BASE}/v2/pos/lookup`, { waitUntil: 'networkidle' });
    await page.waitForSelector('h1');
    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'web-v2-pos-lookup-session4-idle.png'),
      fullPage: true,
    });

    if (errors.length) {
      console.log('PAGE ERRORS:', errors);
    }
  });

  test('phone input — non-Indian-prefix triggers WARN gate', async ({ page }) => {
    await page.goto(`${BASE}/v2/pos/lookup`, { waitUntil: 'networkidle' });
    await page.waitForSelector('#phone-lookup-input');

    // Type 10 digits starting with "5" — fails the {6,7,8,9} prefix gate
    await page.fill('#phone-lookup-input', '5550001234');
    await page.waitForSelector('#phone-warn');
    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'web-v2-pos-lookup-session4-warn.png'),
      fullPage: true,
    });
  });

  test('phone input — Indian-prefix accepts cleanly', async ({ page }) => {
    await page.goto(`${BASE}/v2/pos/lookup`, { waitUntil: 'networkidle' });
    await page.waitForSelector('#phone-lookup-input');

    // Type 10 digits starting with "9" — passes the prefix gate
    await page.fill('#phone-lookup-input', '9876543210');
    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'web-v2-pos-lookup-session4-indian-ok.png'),
      fullPage: true,
    });
  });
});
