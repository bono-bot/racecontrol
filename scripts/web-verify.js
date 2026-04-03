#!/usr/bin/env node
/**
 * web-verify.js — Playwright headless browser health checker for Racing Point web apps
 *
 * Permanent visual debugging tool. Renders pages in a real browser, checks for:
 * - Error states (404, 500, error boundaries)
 * - Expected content (selectors, text)
 * - Forbidden content (wrong page rendering)
 * - Redirect loops
 * - Console JS errors
 * - Screenshots for visual proof (viewable via Claude Code Read tool)
 *
 * Usage:
 *   node scripts/web-verify.js                     # Check all registered pages
 *   node scripts/web-verify.js staff               # Check one page
 *   node scripts/web-verify.js billing kiosk staff  # Check multiple
 *   node scripts/web-verify.js http://custom:3000   # Custom URL
 *   node scripts/web-verify.js --no-screenshot      # Skip screenshots
 *   node scripts/web-verify.js --json               # JSON output
 *
 * Registered pages: billing, kiosk, staff, admin, portal
 *
 * Exit codes: 0 = all PASS, 1 = FAIL found
 */

const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const SERVER = process.env.RC_SERVER || '192.168.31.23';
const CLOUD = process.env.RC_CLOUD || 'racingpoint.cloud';

// ─── Page Registry ──────────────────────────────────────────────────────────
// Venue (local) + Cloud pages. Run with "all" to check both environments.
const PAGES = {
  billing: {
    name: 'Billing Dashboard',
    url: `http://${SERVER}:3200/billing`,
    viewport: { width: 1024, height: 768 },
    waitMs: 4000,
    expectTitle: /RaceControl|RacingPoint/i,
    expectAny: ['button', 'input', '.text-rp-red', '[data-testid]'],
    expectText: [],
    forbidText: ['Application error', 'Internal Server Error', 'NEXT_NOT_FOUND'],
    forbidUrl: [],
  },
  kiosk: {
    name: 'Customer Kiosk',
    url: `http://${SERVER}:3300/kiosk`,
    viewport: { width: 1920, height: 1080 },
    waitMs: 4000,
    expectTitle: /Kiosk|RacingPoint/i,
    expectAny: ['[data-testid="pod-grid"]', 'header', 'main'],
    expectText: ['Staff Login'],
    forbidText: ['Application error', 'Internal Server Error'],
    forbidUrl: ['/kiosk/kiosk'],
  },
  staff: {
    name: 'Staff Terminal',
    url: `http://${SERVER}:3300/kiosk/staff`,
    viewport: { width: 1024, height: 768 },
    waitMs: 5000,
    expectTitle: /Kiosk|RacingPoint/i,
    expectAny: ['button', 'h1', 'h2', '[data-testid]'],
    expectText: ['Sign In'],
    forbidText: ['Choose Your Rig', 'Application error'],
    forbidUrl: ['/kiosk/kiosk'],
  },
  admin: {
    name: 'Admin Dashboard',
    url: `http://${SERVER}:3201`,
    viewport: { width: 1440, height: 900 },
    waitMs: 4000,
    expectTitle: /Admin|RaceControl|RacingPoint/i,
    expectAny: [],
    expectText: [],
    forbidText: ['Cannot GET', 'Application error', 'Internal Server Error'],
    forbidUrl: [],
  },
  portal: {
    name: 'Portal Status',
    url: `http://${SERVER}:8080/status`,
    viewport: { width: 1024, height: 768 },
    waitMs: 3000,
    expectTitle: /.*/,
    expectAny: [],
    expectText: [],
    forbidText: ['Cannot GET', 'Internal Server Error'],
    forbidUrl: [],
  },

  // ─── Cloud Pages ────────────────────────────────────────────────────────
  'cloud-app': {
    name: 'Cloud PWA (app.racingpoint.cloud)',
    url: `https://app.${CLOUD}`,
    viewport: { width: 375, height: 812 },  // mobile viewport — it's a PWA
    waitMs: 5000,
    expectTitle: /RacingPoint/i,
    expectAny: ['button', 'div', 'section'],
    expectText: [],
    forbidText: ['Application error', 'Internal Server Error', 'Cannot GET', '502 Bad Gateway'],
    forbidUrl: [],
  },
  'cloud-admin': {
    name: 'Cloud Admin (admin.racingpoint.cloud)',
    url: `https://admin.${CLOUD}`,
    viewport: { width: 1440, height: 900 },
    waitMs: 4000,
    expectTitle: /Admin|RacingPoint/i,
    expectAny: [],
    expectText: [],
    forbidText: ['Cannot GET', 'Application error', '502 Bad Gateway'],
    forbidUrl: [],
  },
  'cloud-web': {
    name: 'Cloud Web Dashboard',
    url: `https://${CLOUD}`,
    viewport: { width: 1024, height: 768 },
    waitMs: 4000,
    expectTitle: /RacingPoint|RaceControl/i,
    expectAny: [],
    expectText: [],
    forbidText: ['Cannot GET', 'Application error', '502 Bad Gateway'],
    forbidUrl: [],
  },
};

// ─── Check Engine ───────────────────────────────────────────────────────────

async function checkPage(browser, cfg, screenshotDir) {
  const r = {
    name: cfg.name, url: cfg.url, pass: true,
    checks: [], warnings: [], consoleErrors: [],
    finalUrl: '', title: '', screenshot: null,
  };
  let redirects = 0;

  const ctx = await browser.newContext({
    viewport: cfg.viewport || { width: 1024, height: 768 },
    ignoreHTTPSErrors: true,
  });
  const page = await ctx.newPage();

  page.on('console', m => { if (m.type() === 'error') r.consoleErrors.push(m.text().slice(0, 200)); });
  page.on('pageerror', e => r.consoleErrors.push(e.message.slice(0, 200)));
  page.on('response', res => { if (res.status() >= 300 && res.status() < 400) redirects++; });

  // Navigate
  try {
    await page.goto(cfg.url, { waitUntil: 'networkidle', timeout: 12000 }).catch(() =>
      page.goto(cfg.url, { waitUntil: 'domcontentloaded', timeout: 8000 })
    );
  } catch (err) {
    r.pass = false;
    r.checks.push({ name: 'navigate', pass: false, detail: err.message.split('\n')[0] });
    await ctx.close();
    return r;
  }

  await page.waitForTimeout(cfg.waitMs || 3000);

  r.finalUrl = page.url();
  r.title = await page.title();
  const body = await page.textContent('body').catch(() => '');

  // 1. Redirect loop
  if (redirects > 8) {
    r.pass = false;
    r.checks.push({ name: 'redirect-loop', pass: false, detail: `${redirects} redirects` });
  } else {
    r.checks.push({ name: 'redirect-loop', pass: true, detail: `${redirects} redirects` });
  }

  // 2. Title
  if (cfg.expectTitle) {
    const ok = cfg.expectTitle.test(r.title);
    r.checks.push({ name: 'title', pass: ok, detail: `"${r.title}"` });
    if (!ok) r.pass = false;
  }

  // 3. Expected selectors (any one visible)
  if (cfg.expectAny?.length) {
    let found = null;
    for (const sel of cfg.expectAny) {
      if (await page.locator(sel).first().isVisible().catch(() => false)) { found = sel; break; }
    }
    r.checks.push({ name: 'selector', pass: !!found, detail: found || `none of [${cfg.expectAny.join(', ')}]` });
    if (!found) r.pass = false;
  }

  // 4. Expected text
  for (const t of (cfg.expectText || [])) {
    const has = body.includes(t);
    r.checks.push({ name: `text:${t}`, pass: has, detail: has ? 'found' : 'MISSING' });
    if (!has) r.pass = false;
  }

  // 5. Forbidden text
  for (const t of (cfg.forbidText || [])) {
    const has = body.includes(t);
    r.checks.push({ name: `no:${t}`, pass: !has, detail: has ? 'FOUND' : 'absent' });
    if (has) r.pass = false;
  }

  // 6. Forbidden URL
  for (const u of (cfg.forbidUrl || [])) {
    if (r.finalUrl.includes(u)) {
      r.pass = false;
      r.checks.push({ name: `url!=${u}`, pass: false, detail: `final URL: ${r.finalUrl}` });
    }
  }

  // 7. Console errors (warn, don't fail)
  if (r.consoleErrors.length > 0) {
    r.warnings.push(`${r.consoleErrors.length} JS errors`);
  }

  // Screenshot
  if (screenshotDir) {
    const safe = cfg.name.replace(/[^a-zA-Z0-9]/g, '_').toLowerCase();
    const p = path.join(screenshotDir, `${safe}.png`);
    await page.screenshot({ path: p });
    r.screenshot = p;
  }

  await ctx.close();
  return r;
}

// ─── Main ───────────────────────────────────────────────────────────────────

async function main() {
  const rawArgs = process.argv.slice(2);
  const flags = rawArgs.filter(a => a.startsWith('--'));
  const targets = rawArgs.filter(a => !a.startsWith('--'));
  const jsonMode = flags.includes('--json');
  const noScreenshot = flags.includes('--no-screenshot');

  // Build page list
  let pages = [];
  if (targets.length === 0) {
    pages = Object.values(PAGES);
  } else {
    for (const t of targets) {
      if (PAGES[t]) {
        pages.push(PAGES[t]);
      } else if (t.startsWith('http')) {
        pages.push({
          name: `Custom: ${t}`, url: t,
          viewport: { width: 1024, height: 768 }, waitMs: 4000,
          expectTitle: /.*/, expectAny: [], expectText: [],
          forbidText: ['Application error', 'Internal Server Error'],
          forbidUrl: [],
        });
      } else {
        if (!jsonMode) console.error(`Unknown target: ${t}. Available: ${Object.keys(PAGES).join(', ')}`);
      }
    }
  }
  if (pages.length === 0) process.exit(1);

  // Screenshot dir
  const screenshotDir = noScreenshot ? null : path.join(
    process.env.TEMP || '/tmp',
    `web-verify-${Date.now()}`
  );
  if (screenshotDir) fs.mkdirSync(screenshotDir, { recursive: true });

  if (!jsonMode) console.log(`\n  Web Verify — ${pages.length} page(s)\n`);

  const browser = await chromium.launch({ headless: true });
  const results = [];

  for (const pg of pages) {
    if (!jsonMode) process.stdout.write(`  ${pg.name} ... `);
    const r = await checkPage(browser, pg, screenshotDir);
    results.push(r);

    if (!jsonMode) {
      console.log(r.pass ? 'PASS' : 'FAIL');
      for (const c of r.checks) {
        console.log(`    ${c.pass ? 'v' : 'x'} ${c.name}: ${c.detail}`);
      }
      if (r.warnings.length) console.log(`    ! ${r.warnings.join(', ')}`);
      if (r.screenshot) console.log(`    screenshot: ${r.screenshot}`);
      console.log();
    }
  }

  await browser.close();

  // Summary
  const pass = results.filter(r => r.pass).length;
  const fail = results.filter(r => !r.pass).length;

  if (jsonMode) {
    console.log(JSON.stringify({ pass, fail, screenshotDir, results }, null, 2));
  } else {
    console.log(`  ${'—'.repeat(50)}`);
    console.log(`  Results: ${pass} PASS, ${fail} FAIL`);
    if (fail > 0) {
      console.log('  Failed:');
      for (const r of results.filter(r => !r.pass)) {
        const failedChecks = r.checks.filter(c => !c.pass).map(c => c.name);
        console.log(`    ${r.name}: ${failedChecks.join(', ')}`);
      }
    }
    if (screenshotDir) console.log(`  Screenshots: ${screenshotDir}`);
    console.log(`  ${'—'.repeat(50)}\n`);
  }

  process.exit(fail > 0 ? 1 : 0);
}

main().catch(e => { console.error(`Fatal: ${e.message}`); process.exit(1); });
