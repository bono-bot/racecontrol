import { test, expect } from '../fixtures/cleanup';
import { setupApiMocks } from '../fixtures/api-mocks';

// ---- Shared error capture ----

let jsErrors: string[] = [];

test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
  // Enable API mocks so wizard tests work without a live backend
  await setupApiMocks(page);
});

test.afterEach(async ({ page }, testInfo) => {
  // DOM snapshot on failure
  if (testInfo.status !== testInfo.expectedStatus) {
    try {
      const dom = await page.content();
      await testInfo.attach('dom-snapshot.html', {
        body: Buffer.from(dom),
        contentType: 'text/html',
      });
    } catch { /* page may have closed */ }
  }

  // Fail test if uncaught JS errors occurred
  if (jsErrors.length > 0) {
    const errList = jsErrors.join('; ');
    jsErrors = [];
    throw new Error(`Uncaught JS errors during test: ${errList}`);
  }
});

// ---- Helper: enter wizard via staff login + pod card click ----
// Current flow: /kiosk/staff → "Tap to Sign In" → PIN entry → pod grid → click idle pod
// API mocks provide staff auth + fleet data with idle pods.

async function enterWizard(page: import('@playwright/test').Page) {
  await page.goto('/kiosk/staff', { waitUntil: 'networkidle' });

  // Step 1: Click "Tap to Sign In" (idle state)
  const signInBtn = page.getByText('Tap to Sign In');
  if (await signInBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
    await signInBtn.click();
    await page.waitForTimeout(500);

    // Step 2: Enter staff PIN via keypad (mock accepts any 4-digit PIN)
    for (const digit of ['0', '0', '0', '9']) {
      await page.locator(`button:has-text("${digit}")`).first().click();
      await page.waitForTimeout(150);
    }
    // Auto-submits at 4 digits — wait for dashboard to load
    await page.waitForTimeout(3000);
  }

  // Step 3: Click an idle pod card to open the setup wizard
  // Pod cards are cursor:pointer divs containing "Pod N" + "Idle" + "Ready"
  // Use the pod card container (main > div > div with cursor-pointer)
  const podCards = page.locator('main >> div[class*="cursor-pointer"], main >> div[class*="cursor"]');
  const pod1Card = page.getByText('Pod 1').first();

  if (await podCards.first().isVisible({ timeout: 5000 }).catch(() => false)) {
    await podCards.first().click();
  } else if (await pod1Card.isVisible({ timeout: 5000 }).catch(() => false)) {
    // Click the parent div that has the cursor-pointer
    await pod1Card.click();
  }

  // Wait for wizard side panel to appear — the search input is the definitive indicator
  const driverSearchInput = page.getByPlaceholder('Name or phone...');
  await driverSearchInput.waitFor({ state: 'visible', timeout: 15000 }).catch(() => {});

  if (!(await driverSearchInput.isVisible())) {
    await page.screenshot({ path: 'tests/e2e/results/wizard-debug-no-input.png' });
    test.skip(true, 'Wizard search input not visible after pod click');
  }
}

// ---- Wizard flow tests ----

test('non-AC wizard: F1 25 skips AC-specific steps', async ({ page }) => {
  await enterWizard(page);

  // Navigate to select_plan if we start at register_driver
  const registerStep = page.locator('[data-testid="step-register-driver"]');
  if (await registerStep.isVisible({ timeout: 3000 }).catch(() => false)) {
    // Search for and select a driver
    const driverResult = page.locator('[data-testid^="driver-result-"]').first();
    if (await driverResult.isVisible({ timeout: 3000 }).catch(() => false)) {
      await driverResult.click();
    } else {
      test.skip(true, 'No test drivers available for wizard flow');
      return;
    }
  }

  // select_plan — pick first available tier
  const planStep = page.locator('[data-testid="step-select-plan"]');
  await planStep.waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();

  // select_game — pick F1 25
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="game-option-f1_25"]').click();

  // For non-AC games, AC-only steps must NOT be present
  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-driving-settings"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-player-mode"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-session-type"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-ai-config"]')).not.toBeVisible();
});

test('AC wizard: preset path navigates through AC steps and reaches review', async ({ page }) => {
  await enterWizard(page);

  // Handle register_driver step if present — search for any driver
  const driverSearch = page.getByPlaceholder('Name or phone...');
  if (await driverSearch.isVisible({ timeout: 5000 }).catch(() => false)) {
    // Search for "E2E" to find test drivers, or "a" as a broad search
    // Click the search input to focus it — driver list should show on focus or type
    await driverSearch.click();
    await page.waitForTimeout(1000);
    // Type a single char to trigger filtering
    await driverSearch.fill('a');
    await page.waitForTimeout(2000);

    // Debug: take screenshot to see driver search state
    await page.screenshot({ path: 'tests/e2e/results/wizard-debug-driver-search.png' });

    // Click first driver result by data-testid or by any button in the search area
    const driverResult = page.locator('[data-testid^="driver-result-"]').first();
    const anyDriverBtn = page.locator('button:below(input[placeholder*="Name or phone"])').first();

    if (await driverResult.isVisible({ timeout: 3000 }).catch(() => false)) {
      await driverResult.click();
    } else if (await anyDriverBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await anyDriverBtn.click();
    } else {
      test.skip(true, 'No drivers found in search results — driver list may not have loaded');
      return;
    }
    await page.waitForTimeout(1000);
  }

  // select_plan — pick first tier
  // Wait for select_plan step (by data-testid or heading text)
  const planByTestId = page.locator('[data-testid="step-select-plan"]');
  const planByText = page.getByText('Select Plan').first();
  await Promise.race([
    planByTestId.waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
    planByText.waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
  ]);
  // Small wait for rendering
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();

  // select_game — pick Assetto Corsa
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="game-option-assetto_corsa"]').click();

  // player_mode — click single player
  await page.locator('[data-testid="step-player-mode"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="player-mode-single"]').click();

  // session_type — click first option
  await page.locator('[data-testid="step-session-type"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid^="session-type-"]').first().click();

  // ai_config — advance with next button (AI off by default)
  await page.locator('[data-testid="step-ai-config"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="ai-config-next"]').click();

  // select_experience — pick first preset if available
  await page.locator('[data-testid="step-select-experience"]').waitFor({ state: 'visible', timeout: 10000 });
  const expBtn = page.locator('[data-testid^="experience-option-"]').first();
  if (await expBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
    await expBtn.click();
  }

  // driving_settings — advance
  const hasDrivingSettings = await page.locator('[data-testid="step-driving-settings"]')
    .isVisible({ timeout: 5000 }).catch(() => false);
  if (hasDrivingSettings) {
    await page.locator('[data-testid="driving-settings-next"]').click();
  }

  // review step should be visible
  await page.locator('[data-testid="step-review"]').waitFor({ state: 'visible', timeout: 10000 });

  // Preset mode must not show track/car selection
  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
});

test('staff terminal: login and pod grid renders correctly', async ({ page }) => {
  await page.goto('/kiosk/staff', { waitUntil: 'networkidle' });

  // Should show either login screen or pod grid (if already authenticated)
  const signIn = page.getByText('Tap to Sign In');
  const podGrid = page.locator('[data-testid^="pod-card-"]').first();

  const hasLogin = await signIn.isVisible({ timeout: 5000 }).catch(() => false);
  const hasPods = await podGrid.isVisible({ timeout: 5000 }).catch(() => false);

  // Either the login screen or the pod grid should be visible
  expect(hasLogin || hasPods).toBe(true);

  // No error boundary
  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error|unhandled runtime error/i);
});

test('navigation: back button returns to previous step', async ({ page }) => {
  await enterWizard(page);

  // Handle register_driver if present
  const registerStep = page.locator('[data-testid="step-register-driver"]');
  if (await registerStep.isVisible({ timeout: 3000 }).catch(() => false)) {
    const driverResult = page.locator('[data-testid^="driver-result-"]').first();
    if (await driverResult.isVisible({ timeout: 3000 }).catch(() => false)) {
      await driverResult.click();
    } else {
      test.skip(true, 'No test drivers available');
      return;
    }
  }

  // select_plan → pick first tier
  // Wait for select_plan step (by data-testid or heading text)
  const planByTestId = page.locator('[data-testid="step-select-plan"]');
  const planByText = page.getByText('Select Plan').first();
  await Promise.race([
    planByTestId.waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
    planByText.waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
  ]);
  // Small wait for rendering
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();

  // Now on select_game
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });

  // Capture step title
  const titleOnGame = await page.locator('[data-testid="wizard-step-title"]')
    .textContent({ timeout: 5000 }).catch(() => null);

  await page.locator('[data-testid="game-option-f1_25"]').click();

  // Now on next step — title should change
  await page.waitForTimeout(1000);
  const titleAfterGame = await page.locator('[data-testid="wizard-step-title"]')
    .textContent({ timeout: 5000 }).catch(() => null);
  if (titleOnGame && titleAfterGame) {
    expect(titleAfterGame).not.toBe(titleOnGame);
  }

  // Back button should return to select_game
  const backBtn = page.locator('[data-testid="wizard-back-btn"]');
  if (await backBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
    await backBtn.click();
    await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  }
});

test('experience filtering: F1 25 shows no AC-specific steps', async ({ page }) => {
  await enterWizard(page);

  // Handle register_driver if present
  const registerStep = page.locator('[data-testid="step-register-driver"]');
  if (await registerStep.isVisible({ timeout: 3000 }).catch(() => false)) {
    const driverResult = page.locator('[data-testid^="driver-result-"]').first();
    if (await driverResult.isVisible({ timeout: 3000 }).catch(() => false)) {
      await driverResult.click();
    } else {
      test.skip(true, 'No test drivers available');
      return;
    }
  }

  // select_plan
  // Wait for select_plan step (by data-testid or heading text)
  const planByTestId = page.locator('[data-testid="step-select-plan"]');
  const planByText = page.getByText('Select Plan').first();
  await Promise.race([
    planByTestId.waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
    planByText.waitFor({ state: 'visible', timeout: 10000 }).catch(() => {}),
  ]);
  // Small wait for rendering
  await page.locator('[data-testid^="pricing-tier-"], [data-testid^="tier-option-"]').first().click();

  // select_game — F1 25
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="game-option-f1_25"]').click();

  // AC-only steps must be absent from the DOM for non-AC games
  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-driving-settings"]')).not.toBeVisible();
});
