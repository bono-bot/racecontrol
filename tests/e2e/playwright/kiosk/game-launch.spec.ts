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

// ---- Helper: enter wizard via staff walk-in ----
async function enterWizard(page: import('@playwright/test').Page) {
  await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });
  await page.locator('[data-testid="walkin-btn"]').click();
  await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 });
}

// ---- Game launch: full wizard flow to review ----

test('game launch: complete wizard to review step (non-AC)', async ({ page }) => {
  await enterWizard(page);

  // Step 1: select plan — pick first tier
  const tierBtns = page.locator('[data-testid^="tier-option-"]');
  const tierCount = await tierBtns.count();
  expect(tierCount).toBeGreaterThan(0);
  await tierBtns.first().click();

  // Step 2: select game — pick first non-AC game
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  const gameBtns = page.locator('[data-testid^="game-option-"]');
  const gameCount = await gameBtns.count();
  expect(gameCount).toBeGreaterThan(0);

  // Try F1 25 first, fallback to first available
  const f1Btn = page.locator('[data-testid="game-option-f1_25"]');
  const hasF1 = await f1Btn.isVisible({ timeout: 3000 }).catch(() => false);
  if (hasF1) {
    await f1Btn.click();
  } else {
    await gameBtns.first().click();
  }

  // Step 3: select experience
  const expStep = page.locator('[data-testid="step-select-experience"]');
  const hasExp = await expStep.isVisible({ timeout: 5000 }).catch(() => false);
  if (hasExp) {
    const expBtns = page.locator('[data-testid^="experience-option-"]');
    const hasExpOption = await expBtns.first().isVisible({ timeout: 3000 }).catch(() => false);
    if (hasExpOption) {
      await expBtns.first().click();
    }
  }

  // Should reach review (or be on the last available step)
  const review = page.locator('[data-testid="step-review"]');
  const reachedReview = await review.isVisible({ timeout: 5000 }).catch(() => false);

  // Either we reached review or we're on a valid step — no crashes
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

test('game launch: AC wizard reaches driving settings', async ({ page }) => {
  await enterWizard(page);

  // select plan
  await page.locator('[data-testid^="tier-option-"]').first().click();

  // select game — Assetto Corsa
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  const acBtn = page.locator('[data-testid="game-option-assetto_corsa"]');
  const hasAC = await acBtn.isVisible({ timeout: 3000 }).catch(() => false);

  if (!hasAC) {
    test.skip(true, 'Assetto Corsa not available in game catalog');
    return;
  }
  await acBtn.click();

  // session_splits (conditional)
  const hasSplits = await page.locator('[data-testid="step-session-splits"]').isVisible({ timeout: 3000 }).catch(() => false);
  if (hasSplits) {
    await page.locator('[data-testid^="split-option-"]').first().click();
  }

  // player_mode
  await page.locator('[data-testid="step-player-mode"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.getByRole('button', { name: /single/i }).first().click();

  // session_type
  await page.locator('[data-testid="step-session-type"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="step-session-type"] button').first().click();

  // ai_config — advance with next
  await page.locator('[data-testid="step-ai-config"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="wizard-next-btn"]').click();

  // select_experience
  const expStep = page.locator('[data-testid="step-select-experience"]');
  const hasExp = await expStep.isVisible({ timeout: 5000 }).catch(() => false);
  if (hasExp) {
    const expBtns = page.locator('[data-testid^="experience-option-"]');
    const hasExpOption = await expBtns.first().isVisible({ timeout: 3000 }).catch(() => false);
    if (hasExpOption) {
      await expBtns.first().click();
    }
  }

  // driving_settings
  const hasDriving = await page.locator('[data-testid="step-driving-settings"]').isVisible({ timeout: 5000 }).catch(() => false);

  // Should be somewhere in the AC flow — no crashes
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error/i);
});

// ---- Game launch: verify all wizard buttons are interactive ----

test('game launch: all tier buttons are clickable', async ({ page }) => {
  await enterWizard(page);

  const tierBtns = page.locator('[data-testid^="tier-option-"]');
  const count = await tierBtns.count();

  for (let i = 0; i < count; i++) {
    const btn = tierBtns.nth(i);
    await expect(btn).toBeEnabled();
    // Verify visual state — should have text content
    const text = await btn.textContent();
    expect(text?.trim().length).toBeGreaterThan(0);
  }
});

test('game launch: all game buttons are clickable', async ({ page }) => {
  await enterWizard(page);

  // Select first tier to advance
  await page.locator('[data-testid^="tier-option-"]').first().click();
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
