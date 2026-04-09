/**
 * v47.0 Phase 360 visual evidence: wallet topup preset SSOT.
 *
 * Captures screenshots of the PWA /wallet/topup page and the POS
 * WalletTopupModal (opened from /billing). These surfaces were not in the
 * existing page-crawler routes manifest, so before/after drift detection for
 * wallet topup preset amounts is not covered by the regular crawler.
 *
 * This spec:
 *   1. Navigates to the PWA /wallet/topup page (unauthenticated — will land
 *      on /login) and captures the current render
 *   2. Navigates to the POS /billing page (unauthenticated — will land on
 *      /login) and captures the current render
 *
 * Both captures are WITHOUT login. The goal is to prove the pages RENDER
 * without runtime errors after v47.0 Phase 360 wiring. A deployed-and-signed-in
 * visual check is still required after the first live deploy — the code
 * changes fetch from /api/v1/wallet/topup-presets at component mount, and
 * that only fires once logged in. Source: PITFALLS-v47.md #10 (probes that lie).
 *
 * Run: npx playwright test --config tests/page-crawler/playwright.config.ts \
 *        tests/page-crawler/wallet-topup-v47-360.spec.ts
 */

import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';

const SCREENSHOT_DIR = path.join(__dirname, '../screenshots/v47-phase-360');
const PWA_URL = process.env.PWA_URL ?? 'http://192.168.31.23:3000';
const WEB_URL = process.env.WEB_URL ?? 'http://192.168.31.23:3200';

test.beforeAll(() => {
  fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
});

test.describe('v47.0 Phase 360 — wallet topup SSOT visual evidence', () => {
  test('PWA /wallet/topup renders without runtime errors', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('pageerror', (err) => consoleErrors.push(err.message));
    page.on('console', (msg) => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });

    // Navigate to the PWA wallet topup page — unauthenticated will redirect
    // to /login, but the component module must still load without errors.
    const response = await page.goto(`${PWA_URL}/wallet/topup`, {
      waitUntil: 'domcontentloaded',
      timeout: 30_000,
    });

    expect(response, 'PWA should respond to /wallet/topup').not.toBeNull();

    // Capture the rendered state (either the topup page or /login redirect)
    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'pwa-wallet-topup.png'),
      fullPage: true,
    });

    // The page MUST NOT throw runtime JS errors — this is the core proof that
    // the v47.0 Phase 360 code changes (topupPresets fetch, setPresetAmounts,
    // FALLBACK_PRESET_AMOUNTS_PAISE) don't break the module.
    expect(
      consoleErrors.filter((e) => !e.includes('WebSocket') && !e.includes('favicon')),
      `PWA /wallet/topup must render without runtime errors. Got: ${consoleErrors.join(', ')}`,
    ).toHaveLength(0);
  });

  test('POS /billing renders without runtime errors', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('pageerror', (err) => consoleErrors.push(err.message));
    page.on('console', (msg) => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });

    const response = await page.goto(`${WEB_URL}/billing`, {
      waitUntil: 'domcontentloaded',
      timeout: 30_000,
    });

    expect(response, 'POS should respond to /billing').not.toBeNull();

    await page.screenshot({
      path: path.join(SCREENSHOT_DIR, 'pos-billing.png'),
      fullPage: true,
    });

    // WalletTopupModal is imported by /billing page and compiled into the
    // bundle. If the v47.0 Phase 360 changes broke the component, the whole
    // /billing page fails to hydrate — this test catches that.
    expect(
      consoleErrors.filter((e) => !e.includes('WebSocket') && !e.includes('favicon')),
      `POS /billing must render without runtime errors. Got: ${consoleErrors.join(', ')}`,
    ).toHaveLength(0);
  });
});
