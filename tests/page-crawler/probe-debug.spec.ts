/**
 * Ad-hoc probe: verify /kiosk/debug hydrates and fetches real data.
 * Not part of the regular crawl — invoked manually.
 *
 *   npx playwright test --config tests/page-crawler/playwright.config.ts probe-debug.spec.ts --project=kiosk
 */

import { test, expect } from '@playwright/test';
import { ensureAuth } from './auth-setup';
import * as path from 'path';
import * as fs from 'fs';

const DEBUG_PATH = '/kiosk/debug';

test.describe('kiosk debug page probe', () => {
  test('hydrate + api calls + screenshot', async ({ browser, baseURL }) => {
    await ensureAuth(baseURL!); // ensures cookie is available

    // The kiosk app requires 3 keys in sessionStorage (kiosk_staff_token,
    // kiosk_staff_name, kiosk_staff_id) and the kiosk_staff_jwt cookie.
    // auth-setup.ts only provides the cookie + a localStorage token (wrong
    // storage), so we mint fresh and inject everything directly.
    const pinResp = await (await import('http')).default as never; // unused; use fetch below
    const apiBase = process.env.RC_API_URL ?? 'http://192.168.31.23:8080';
    const pinRes = await fetch(`${apiBase}/api/v1/staff/validate-pin`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ pin: process.env.STAFF_PIN ?? '7080' }),
    });
    const pinBody = (await pinRes.json()) as {
      token: string;
      staff_id: string;
      staff_name: string;
    };
    console.log('[probe] minted staff token:', pinBody.staff_name, pinBody.staff_id);

    const context = await browser.newContext({
      viewport: { width: 1920, height: 1080 },
    });

    // Inject cookie at the right origin
    const cookieUrl = new URL(baseURL!);
    await context.addCookies([
      {
        name: 'kiosk_staff_jwt',
        value: pinBody.token,
        domain: cookieUrl.hostname,
        path: '/',
        expires: Math.floor(Date.now() / 1000) + 1800,
        httpOnly: false,
        secure: false,
        sameSite: 'Strict',
      },
    ]);

    // Inject sessionStorage BEFORE any page script runs
    await context.addInitScript(
      ({ token, id, name }: { token: string; id: string; name: string }) => {
        try {
          window.sessionStorage.setItem('kiosk_staff_token', token);
          window.sessionStorage.setItem('kiosk_staff_id', id);
          window.sessionStorage.setItem('kiosk_staff_name', name);
        } catch {
          /* ignore */
        }
      },
      { token: pinBody.token, id: pinBody.staff_id, name: pinBody.staff_name },
    );

    const page = await context.newPage();

    const consoleMessages: Array<{ type: string; text: string }> = [];
    page.on('console', (msg) => {
      consoleMessages.push({ type: msg.type(), text: msg.text() });
    });

    const pageErrors: string[] = [];
    page.on('pageerror', (err) => {
      pageErrors.push(err.message);
    });

    const apiCalls: Array<{ url: string; status: number; method: string }> = [];
    page.on('response', (resp) => {
      const url = resp.url();
      // Track API calls that matter for the debug page
      if (
        url.includes('/debug/') ||
        url.includes('/pods/') ||
        url.includes('/fleet/') ||
        url.includes('/api/v1/')
      ) {
        apiCalls.push({ url, status: resp.status(), method: resp.request().method() });
      }
    });

    // Navigate to :3300 direct — localStorage in the saved storageState is
    // scoped to origin :3300, so REST calls that read kiosk_staff_token will
    // find it only when visiting the same origin.
    const targetUrl = 'http://192.168.31.23:3300' + DEBUG_PATH;
    console.log('[probe] navigating to', targetUrl);
    const response = await page.goto(targetUrl, { waitUntil: 'domcontentloaded' });
    console.log('[probe] initial HTTP:', response?.status());

    // Give hydration + WS + API loads time to settle
    await page.waitForTimeout(8000);

    // Probe for hydration markers
    const hydrationState = await page.evaluate(() => {
      const pods = document.querySelectorAll('[data-testid="pod-grid"] > div');
      const wsStatus = document.querySelector('[data-testid="ws-status"]');
      const skeletonCount = document.querySelectorAll(
        '[data-testid="pod-grid"] .animate-pulse',
      ).length;
      // Look for any pod-card class indicating real data rendered
      const hydratedPods = document.querySelectorAll('[data-testid^="pod-card-"]');
      // Check body text for key debug-page markers
      const bodyText = document.body.innerText || '';
      return {
        podGridChildren: pods.length,
        skeletonPods: skeletonCount,
        hydratedPodCards: hydratedPods.length,
        wsStatusText: wsStatus?.textContent ?? '(not found)',
        hasDiagnosticsText: bodyText.toLowerCase().includes('diagnostic'),
        hasActivityText: bodyText.toLowerCase().includes('activity'),
        hasPlaybookText: bodyText.toLowerCase().includes('playbook'),
        hasIncidentText: bodyText.toLowerCase().includes('incident'),
        bodyTextSnippet: bodyText.slice(0, 500),
      };
    });

    console.log('[probe] hydration state:', JSON.stringify(hydrationState, null, 2));

    // Capture screenshot
    const outDir = path.join(__dirname, '../../test-results/probe-debug');
    if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true });
    const screenshotPath = path.join(outDir, 'kiosk-debug.png');
    await page.screenshot({ path: screenshotPath, fullPage: true });
    console.log('[probe] screenshot saved:', screenshotPath);

    // Dump captured API calls
    console.log('[probe] API CALLS (' + apiCalls.length + '):');
    for (const c of apiCalls) {
      console.log('  ' + c.status + ' ' + c.method + ' ' + c.url);
    }

    // Dump console
    console.log('[probe] CONSOLE MESSAGES (' + consoleMessages.length + '):');
    for (const m of consoleMessages.slice(0, 40)) {
      console.log('  [' + m.type + '] ' + m.text.slice(0, 200));
    }

    // Dump page errors
    console.log('[probe] PAGE ERRORS (' + pageErrors.length + '):');
    for (const e of pageErrors) {
      console.log('  ' + e);
    }

    // Write structured JSON for later review
    const report = {
      targetUrl,
      initialHttp: response?.status(),
      hydrationState,
      apiCalls,
      consoleMessages,
      pageErrors,
      screenshotPath,
    };
    fs.writeFileSync(
      path.join(outDir, 'probe-report.json'),
      JSON.stringify(report, null, 2),
    );

    await context.close();

    // Assertions are lenient — we want the probe to always finish so we can read the report
    expect(response?.status(), 'initial nav should be 200').toBe(200);
  });
});
