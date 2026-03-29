#!/usr/bin/env node
// verify-pos.js — POS billing dashboard verification via Playwright
//
// Modes:
//   node verify-pos.js local     — headless Chromium against server (default)
//   node verify-pos.js remote    — connect to POS Edge via CDP (requires --remote-debugging-port=9222)
//
// Tests:
//   1. Login with staff PIN
//   2. Billing page loads with pods
//   3. Custom 404 page shows + auto-redirects
//   4. Sidebar navigation works
//   5. Console error capture
//   6. Static chunk verification

const { chromium } = require('playwright');
const http = require('http');

const SERVER_IP = '192.168.31.23';
const POS_IP = '192.168.31.20';
const WEB_PORT = 3200;
const BASE_URL = `http://${SERVER_IP}:${WEB_PORT}`;
const PIN = '1234';
const MODE = process.argv[2] || 'local';
const OUT_DIR = process.argv[3] || 'C:/Users/bono/racingpoint/racecontrol/audit/pos-verify';

const results = [];
const consoleErrors = [];

function log(msg) { console.log(`[verify-pos] ${msg}`); }
function pass(test) { results.push({ test, status: 'PASS' }); log(`PASS: ${test}`); }
function fail(test, reason) { results.push({ test, status: 'FAIL', reason }); log(`FAIL: ${test} — ${reason}`); }

async function run() {
  const fs = require('fs');
  if (!fs.existsSync(OUT_DIR)) fs.mkdirSync(OUT_DIR, { recursive: true });

  let browser, page;

  if (MODE === 'remote') {
    // Connect to POS Edge via Chrome DevTools Protocol
    log(`Connecting to Edge on POS at ${POS_IP}:9222...`);
    try {
      browser = await chromium.connectOverCDP(`http://${POS_IP}:9222`);
      const contexts = browser.contexts();
      page = contexts[0]?.pages()[0];
      if (!page) {
        fail('CDP Connect', 'No pages found on POS Edge');
        return;
      }
      pass('CDP Connect to POS Edge');
    } catch (e) {
      fail('CDP Connect', `Cannot connect to ${POS_IP}:9222 — ${e.message}`);
      log('Hint: Ensure Edge launched with --remote-debugging-port=9222');
      return;
    }
  } else {
    // Local headless Chromium
    log('Launching headless Chromium (local mode)...');
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
    page = await context.newPage();
    pass('Browser launch (local Chromium)');
  }

  // Capture console errors
  page.on('console', msg => {
    if (msg.type() === 'error') consoleErrors.push(msg.text());
  });
  page.on('pageerror', err => consoleErrors.push(err.message));

  // === Test 1: Login ===
  log('Test 1: Login with staff PIN...');
  try {
    await page.goto(`${BASE_URL}/login`, { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(1500);

    for (const digit of PIN.split('')) {
      const btn = page.locator('button', { hasText: new RegExp(`^${digit}$`) });
      await btn.click();
      await page.waitForTimeout(200);
    }
    await page.waitForTimeout(3000);

    const url = page.url();
    if (!url.includes('login')) {
      pass('Login with PIN');
    } else {
      fail('Login with PIN', `Still on ${url}`);
      await page.screenshot({ path: `${OUT_DIR}/01-login-fail.png` });
      await browser.close();
      return printResults();
    }
  } catch (e) {
    fail('Login with PIN', e.message);
    await browser.close();
    return printResults();
  }

  // === Test 2: Billing page loads ===
  log('Test 2: Billing page loads with pods...');
  try {
    await page.goto(`${BASE_URL}/billing`, { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(2000);
    await page.screenshot({ path: `${OUT_DIR}/02-billing.png` });

    const h1 = await page.locator('h1:has-text("Billing")').count();
    const pods = await page.locator('button:has-text("Start Session")').count();

    if (h1 > 0) pass('Billing page H1 visible');
    else fail('Billing page H1 visible', 'H1 "Billing" not found');

    if (pods > 0) pass(`Billing page pods visible (${pods} pods)`);
    else fail('Billing page pods visible', 'No "Start Session" buttons found');
  } catch (e) {
    fail('Billing page loads', e.message);
  }

  // === Test 3: Start Session modal ===
  log('Test 3: Start Session modal opens...');
  try {
    const startBtn = page.locator('button:has-text("Start Session")').first();
    if (await startBtn.count() > 0) {
      await startBtn.click();
      await page.waitForTimeout(1000);
      await page.screenshot({ path: `${OUT_DIR}/03-modal.png` });

      // Check for modal/dialog content
      const modal = await page.locator('[class*="fixed"], [class*="modal"], [role="dialog"]').count();
      if (modal > 0) pass('Start Session modal opens');
      else fail('Start Session modal opens', 'No modal/dialog detected');

      // Close modal (click outside or press Escape)
      await page.keyboard.press('Escape');
      await page.waitForTimeout(500);
    } else {
      fail('Start Session modal opens', 'No Start Session button available');
    }
  } catch (e) {
    fail('Start Session modal opens', e.message);
  }

  // === Test 4: Custom 404 page ===
  log('Test 4: Custom 404 page with auto-redirect...');
  try {
    await page.goto(`${BASE_URL}/billing/nonexistent`, { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(1500);
    await page.screenshot({ path: `${OUT_DIR}/04-404.png` });

    const has404 = await page.locator('h1:has-text("404")').count();
    const hasRedirectText = await page.locator('text=Redirecting to Billing').count();
    const hasButton = await page.locator('button:has-text("Go to Billing Now")').count();

    if (has404 > 0) pass('Custom 404 page shows "404" heading');
    else fail('Custom 404 page', 'No "404" heading found — may be default Next.js 404');

    if (hasRedirectText > 0) pass('Custom 404 shows redirect text');
    else fail('Custom 404 redirect text', 'Missing "Redirecting to Billing" text');

    if (hasButton > 0) pass('Custom 404 has "Go to Billing Now" button');
    else fail('Custom 404 button', 'Missing button');

    // Wait for auto-redirect (3 seconds)
    log('Waiting 4s for auto-redirect...');
    await page.waitForTimeout(4000);
    const afterUrl = page.url();
    await page.screenshot({ path: `${OUT_DIR}/05-404-redirect.png` });

    if (afterUrl.includes('/billing') && !afterUrl.includes('nonexistent')) {
      pass('Custom 404 auto-redirects to /billing');
    } else {
      fail('Custom 404 auto-redirect', `After 4s, URL is ${afterUrl}`);
    }
  } catch (e) {
    fail('Custom 404 page', e.message);
  }

  // === Test 5: Sidebar navigation ===
  log('Test 5: Sidebar navigation...');
  const testRoutes = ['/pods', '/drivers', '/billing'];
  for (const route of testRoutes) {
    try {
      const link = page.locator(`a[href="${route}"]`);
      if (await link.count() > 0) {
        await link.click();
        await page.waitForTimeout(2000);
        if (page.url().endsWith(route) || page.url().includes(route)) {
          pass(`Sidebar nav to ${route}`);
        } else {
          fail(`Sidebar nav to ${route}`, `URL is ${page.url()}`);
        }
      }
    } catch (e) {
      fail(`Sidebar nav to ${route}`, e.message);
    }
  }

  // === Test 6: All routes return 200 ===
  log('Test 6: Route smoke test (HTTP)...');
  const routes = [
    '/', '/billing', '/billing/pricing', '/billing/history', '/pods', '/games',
    '/drivers', '/sessions', '/events', '/leaderboards', '/bookings', '/login',
    '/cameras', '/cameras/playback', '/cafe', '/settings', '/flags', '/ota',
    '/ac-lan', '/ac-sessions', '/ai', '/telemetry', '/kiosk', '/presenter', '/book',
  ];
  let routePass = 0, routeFail = 0;
  for (const route of routes) {
    try {
      const res = await new Promise((resolve, reject) => {
        http.get(`${BASE_URL}${route}`, r => resolve(r.statusCode)).on('error', reject);
      });
      if (res === 200) routePass++;
      else { routeFail++; fail(`Route ${route}`, `HTTP ${res}`); }
    } catch (e) {
      routeFail++;
      fail(`Route ${route}`, e.message);
    }
  }
  if (routeFail === 0) pass(`All ${routePass} routes return 200`);
  else fail(`Route smoke test`, `${routeFail}/${routes.length} failed`);

  // === Test 7: Static chunks ===
  log('Test 7: Static chunk verification...');
  try {
    const html = await page.goto(`${BASE_URL}/billing`).then(() => page.content());
    const chunks = html.match(/\/_next\/static\/chunks\/[^"']+\.js/g) || [];
    let chunkOk = 0;
    for (const chunk of chunks.slice(0, 5)) {
      const status = await new Promise((resolve, reject) => {
        http.get(`${BASE_URL}${chunk}`, r => resolve(r.statusCode)).on('error', reject);
      });
      if (status === 200) chunkOk++;
    }
    if (chunkOk >= 3) pass(`Static chunks OK (${chunkOk}/${Math.min(chunks.length, 5)} verified)`);
    else fail('Static chunks', `Only ${chunkOk} chunks return 200`);
  } catch (e) {
    fail('Static chunks', e.message);
  }

  // === Console errors ===
  if (consoleErrors.length > 0) {
    fail('Console errors', `${consoleErrors.length} errors: ${consoleErrors.slice(0, 3).join('; ')}`);
  } else {
    pass('No console errors');
  }

  await browser.close();
  printResults();
}

function printResults() {
  log('\n========== VERIFICATION RESULTS ==========');
  const passed = results.filter(r => r.status === 'PASS').length;
  const failed = results.filter(r => r.status === 'FAIL').length;

  for (const r of results) {
    const icon = r.status === 'PASS' ? '\u2705' : '\u274C';
    log(`${icon} ${r.test}${r.reason ? ` — ${r.reason}` : ''}`);
  }

  log(`\nTotal: ${passed} PASS, ${failed} FAIL out of ${results.length} tests`);

  if (consoleErrors.length > 0) {
    log(`\nConsole errors captured:`);
    consoleErrors.forEach((e, i) => log(`  ${i + 1}. ${e}`));
  }

  // Write results JSON
  const fs = require('fs');
  fs.writeFileSync(`${OUT_DIR}/results.json`, JSON.stringify({ results, consoleErrors, timestamp: new Date().toISOString() }, null, 2));
  log(`\nResults saved to ${OUT_DIR}/results.json`);

  process.exit(failed > 0 ? 1 : 0);
}

run().catch(e => {
  console.error('Fatal:', e);
  process.exit(2);
});
