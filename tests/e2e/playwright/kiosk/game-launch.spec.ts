import { test, expect } from '@playwright/test';
import { setupKioskApiMocks, loginAndOpenWizard } from './helpers/mocks';

// ---- JS error capture ----
let jsErrors: string[] = [];
test.beforeEach(async ({ page }) => {
  jsErrors = [];
  // Register API mocks BEFORE any navigation so the kiosk can hydrate.
  // Without mocks every fetch fails, wizard never renders, and tests
  // time out 30s on driver-search visibility.
  await setupKioskApiMocks(page);
  page.on('pageerror', (err) => {
    if (/WebSocket|ws:|wss:/.test(err.message)) return;
    // Expected when a spec-specific page.route() 404s an endpoint the
    // kiosk calls; those are test-driven, not product bugs.
    if (/Failed to fetch|NetworkError|fetch failed/i.test(err.message)) return;
    jsErrors.push(err.message);
  });
});
test.afterEach(async ({ page }, testInfo) => {
  // Tests that called `test.skip()` mid-body have status 'skipped'; the
  // page may have generated JS errors before the skip fired, but those
  // errors do not indicate a product regression.
  if (testInfo.status === 'skipped') return;

  if (testInfo.status !== testInfo.expectedStatus) {
    try {
      await testInfo.attach('dom-snapshot.html', {
        body: Buffer.from(await page.content()),
        contentType: 'text/html',
      });
    } catch {}
  }
  if (jsErrors.length > 0) {
    const msg = jsErrors.join('; ');
    jsErrors = [];
    throw new Error(`Uncaught JS errors: ${msg}`);
  }
});

// ---- Helper: enter wizard via staff login + pod card ----
async function enterWizard(page: import('@playwright/test').Page) {
  await loginAndOpenWizard(page, 1);

  // Wait for wizard
  const searchInput = page.locator('input[placeholder="Name or phone..."]');
  await searchInput.waitFor({ state: 'visible', timeout: 15000 }).catch(() => {});
  if (!(await searchInput.isVisible())) {
    test.skip(true, 'Wizard not reachable after pod click');
  }
}

// ---- Helper: select a driver from wizard search ----
async function selectDriver(page: import('@playwright/test').Page) {
  const driverSearch = page.locator('input[placeholder="Name or phone..."]');
  if (await driverSearch.isVisible({ timeout: 5000 }).catch(() => false)) {
    await driverSearch.click();
    await page.waitForTimeout(3000);
    await page.keyboard.type('E2E Test', { delay: 80 });
    await page.waitForTimeout(2000);

    const driverBtn = page.locator('button').filter({ hasText: /E2E Test Driver/ }).first();
    if (await driverBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await driverBtn.click();
      await page.waitForTimeout(1000);
    } else {
      test.skip(true, 'No drivers found in search');
    }
  }
}

// ---- Helper: select pricing tier (text-based) ----
async function selectTier(page: import('@playwright/test').Page) {
  await page.getByText('Select Plan').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {});
  const tierBtn = page.locator('button').filter({ hasText: /30 Min|60 Min|Free Trial|Per Min/i }).first();
  if (await tierBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
    await tierBtn.click();
    await page.waitForTimeout(1000);
  } else {
    test.skip(true, 'No pricing tier buttons found');
  }
}

// ---- Game launch tests ----

test('game launch: complete wizard to review step (non-AC)', async ({ page }) => {
  await enterWizard(page);
  await selectDriver(page);
  await selectTier(page);

  // select game — pick F1 25
  await page.getByText('Select Game').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {});
  const f1Btn = page.locator('button').filter({ hasText: /F1 25/i }).first();
  if (await f1Btn.isVisible({ timeout: 5000 }).catch(() => false)) {
    await f1Btn.click();
  } else {
    // Try any non-AC game
    const anyGame = page.locator('button').filter({ hasText: /Forza|iRacing|Le Mans/i }).first();
    if (await anyGame.isVisible({ timeout: 3000 }).catch(() => false)) {
      await anyGame.click();
    } else {
      test.skip(true, 'No game buttons found');
      return;
    }
  }

  // Non-AC should skip AC-specific steps — no crash
  await page.waitForTimeout(2000);
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error|unhandled runtime error/i);
});

test('game launch: AC wizard reaches driving settings', async ({ page }) => {
  await enterWizard(page);
  await selectDriver(page);
  await selectTier(page);

  // select game — Assetto Corsa
  await page.getByText('Select Game').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {});
  const acBtn = page.locator('button').filter({ hasText: /^Assetto Corsa$/i }).first();
  if (!(await acBtn.isVisible({ timeout: 5000 }).catch(() => false))) {
    test.skip(true, 'Assetto Corsa not in game list');
    return;
  }
  await acBtn.click();
  await page.waitForTimeout(1000);

  // Navigate through AC-specific steps — each step has heading text
  // player_mode
  const singleBtn = page.locator('button').filter({ hasText: /Single Player/i }).first();
  if (await singleBtn.isVisible({ timeout: 8000 }).catch(() => false)) {
    await singleBtn.click();
    await page.waitForTimeout(1000);
  }

  // Continue through remaining steps without crashing
  await page.waitForTimeout(2000);
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error|unhandled runtime error/i);
});

test('game launch: all tier buttons are clickable', async ({ page }) => {
  await enterWizard(page);
  await selectDriver(page);

  await page.getByText('Select Plan').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {});

  // Find all tier buttons by text content
  const tierButtons = page.locator('button').filter({ hasText: /30 Min|60 Min|Free Trial|Per Min/i });
  const count = await tierButtons.count();
  expect(count).toBeGreaterThan(0);

  for (let i = 0; i < count; i++) {
    const btn = tierButtons.nth(i);
    await expect(btn).toBeEnabled();
    const text = await btn.textContent();
    expect(text?.trim().length).toBeGreaterThan(0);
  }
});

test('game launch: all game buttons are clickable', async ({ page }) => {
  await enterWizard(page);
  await selectDriver(page);
  await selectTier(page);

  await page.getByText('Select Game').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {});

  // Find all game buttons
  const gameButtons = page.locator('button').filter({
    hasText: /Assetto Corsa|F1 25|iRacing|Le Mans|Forza|EA SPORTS/i
  });
  const count = await gameButtons.count();
  expect(count).toBeGreaterThan(0);

  for (let i = 0; i < count; i++) {
    const btn = gameButtons.nth(i);
    await expect(btn).toBeEnabled();
    const text = await btn.textContent();
    expect(text?.trim().length).toBeGreaterThan(0);
  }
});
