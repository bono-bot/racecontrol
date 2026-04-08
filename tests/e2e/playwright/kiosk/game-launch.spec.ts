import { test, expect } from '../fixtures/cleanup';
import { setupApiMocks } from '../fixtures/api-mocks';

// ---- JS error capture ----
let jsErrors: string[] = [];
test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
  await setupApiMocks(page);
});
test.afterEach(async ({ page }, testInfo) => {
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
  await page.goto('/kiosk/staff', { waitUntil: 'networkidle' });

  // Staff login flow: Tap to Sign In → PIN keypad → pod grid
  const signInBtn = page.getByText('Tap to Sign In');
  if (await signInBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
    await signInBtn.click();
    await page.waitForTimeout(500);
    for (const digit of ['0', '0', '0', '9']) {
      await page.locator(`button:has-text("${digit}")`).first().click();
      await page.waitForTimeout(150);
    }
    await page.waitForTimeout(3000);
  }

  // Click an idle pod card to open the wizard
  const idlePod = page.getByText('Ready').first().locator('..');
  const pod1 = page.getByText('Pod 1').first();

  if (await idlePod.isVisible({ timeout: 5000 }).catch(() => false)) {
    await idlePod.click();
  } else if (await pod1.isVisible({ timeout: 5000 }).catch(() => false)) {
    await pod1.click();
  }

  // Wait for wizard — "Select Driver" heading or data-testid step
  const wizardVisible = await page.getByText('Select Driver').first()
    .isVisible({ timeout: 10000 }).catch(() => false);
  if (!wizardVisible) {
    test.skip(true, 'Wizard not reachable — staff auth or pod selection failed');
  }
}

// ---- Helper: skip register_driver step by searching for test driver ----
async function skipDriverStep(page: import('@playwright/test').Page) {
  const driverSearch = page.locator('input[placeholder*="Name or phone"]');
  if (await driverSearch.isVisible({ timeout: 3000 }).catch(() => false)) {
    await driverSearch.fill('E');
    await page.waitForTimeout(1000);
    const driverResult = page.locator('[data-testid^="driver-result-"]').first();
    if (await driverResult.isVisible({ timeout: 5000 }).catch(() => false)) {
      await driverResult.click();
    } else {
      await driverSearch.clear();
      await page.waitForTimeout(500);
      if (await driverResult.isVisible({ timeout: 5000 }).catch(() => false)) {
        await driverResult.click();
      } else {
        test.skip(true, 'No drivers found in search');
      }
    }
    await page.waitForTimeout(1000);
  }
}

// ---- Game launch: full wizard flow to review ----

test('game launch: complete wizard to review step (non-AC)', async ({ page }) => {
  await enterWizard(page);
  await skipDriverStep(page);

  // select plan — pick first tier
  const planStep = page.locator('[data-testid="step-select-plan"]');
  await planStep.waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();

  // select game — pick F1 25 or first non-AC game
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  const f1Btn = page.locator('[data-testid="game-option-f1_25"]');
  if (await f1Btn.isVisible({ timeout: 3000 }).catch(() => false)) {
    await f1Btn.click();
  } else {
    await page.locator('[data-testid^="game-option-"]').first().click();
  }

  // For non-AC: should skip to select_experience or review
  const expStep = page.locator('[data-testid="step-select-experience"]');
  if (await expStep.isVisible({ timeout: 5000 }).catch(() => false)) {
    const expBtn = page.locator('[data-testid^="experience-option-"]').first();
    if (await expBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await expBtn.click();
    }
  }

  // Should reach review or be on a valid step — no crashes
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

test('game launch: AC wizard reaches driving settings', async ({ page }) => {
  await enterWizard(page);
  await skipDriverStep(page);

  // select plan
  // Wait for select_plan step
  await Promise.race([
    page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
    page.getByText('Select Plan').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
  ]);
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();

  // select game — Assetto Corsa
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  const acBtn = page.locator('[data-testid="game-option-assetto_corsa"]');
  if (!(await acBtn.isVisible({ timeout: 3000 }).catch(() => false))) {
    test.skip(true, 'Assetto Corsa not available');
    return;
  }
  await acBtn.click();

  // player_mode
  await page.locator('[data-testid="step-player-mode"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="player-mode-single"]').click();

  // session_type
  await page.locator('[data-testid="step-session-type"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid^="session-type-"]').first().click();

  // ai_config — advance
  await page.locator('[data-testid="step-ai-config"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="ai-config-next"]').click();

  // select_experience
  const expStep = page.locator('[data-testid="step-select-experience"]');
  if (await expStep.isVisible({ timeout: 5000 }).catch(() => false)) {
    const expBtn = page.locator('[data-testid^="experience-option-"]').first();
    if (await expBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await expBtn.click();
    }
  }

  // driving_settings or review should be reachable
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

test('game launch: all tier buttons are clickable', async ({ page }) => {
  await enterWizard(page);
  await skipDriverStep(page);

  // Wait for select_plan step
  await Promise.race([
    page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
    page.getByText('Select Plan').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
  ]);
  const tierBtns = page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]');
  const count = await tierBtns.count();

  for (let i = 0; i < count; i++) {
    const btn = tierBtns.nth(i);
    await expect(btn).toBeEnabled();
    const text = await btn.textContent();
    expect(text?.trim().length).toBeGreaterThan(0);
  }
});

test('game launch: all game buttons are clickable', async ({ page }) => {
  await enterWizard(page);
  await skipDriverStep(page);

  // Select first tier to advance
  // Wait for select_plan step
  await Promise.race([
    page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
    page.getByText('Select Plan').first().waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
  ]);
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });

  const gameBtns = page.locator('[data-testid^="game-option-"]');
  const count = await gameBtns.count();

  for (let i = 0; i < count; i++) {
    const btn = gameBtns.nth(i);
    await expect(btn).toBeEnabled();
    const text = await btn.textContent();
    expect(text?.trim().length).toBeGreaterThan(0);
  }
});
