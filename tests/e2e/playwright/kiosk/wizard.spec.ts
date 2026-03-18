import { test, expect } from '../fixtures/cleanup';

// ---- Shared error capture ----

let jsErrors: string[] = [];

test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
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

// ---- Helper: enter wizard via staff walk-in (bypasses OTP) ----
// Source: book/page.tsx — isStaffMode adds walkin-btn; handleStaffWalkIn() sets authToken and
// transitions to wizard phase. The URL still lands on the phone screen — walkin-btn click is required.

async function enterWizardViaStaffWalkIn(page: import('@playwright/test').Page) {
  await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });
  await page.locator('[data-testid="walkin-btn"]').click();
  await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 });
}

// ---- Wizard flow tests ----

// BROW-03 — Non-AC wizard: F1 25 shows exactly 4 customer steps
// Expected flow: select_plan → select_game → select_experience → review
// AC-only steps (session_splits, player_mode, session_type, ai_config,
//   select_track, select_car, driving_settings) must never appear.
// Source: useSetupWizard.ts getFlow() lines 131–143 — non-AC filter removes 8 AC-only steps.
test('non-AC wizard: F1 25 shows exactly select_plan → select_game → select_experience → review', async ({ page }) => {
  await enterWizardViaStaffWalkIn(page);

  // Step 1: select_plan — pick first available tier
  await page.locator('[data-testid^="tier-option-"]').first().click();

  // Step 2: select_game — pick F1 25
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="game-option-f1_25"]').click();

  // Step 3: select_experience — AC-only steps must NOT be present
  await page.locator('[data-testid="step-select-experience"]').waitFor({ state: 'visible', timeout: 10000 });

  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-driving-settings"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-session-splits"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-player-mode"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-session-type"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-ai-config"]')).not.toBeVisible();

  // Click first experience if any are configured in the DB; else assert step is still visible
  const expBtn = page.locator('[data-testid^="experience-option-"]').first();
  const hasExperience = await expBtn.isVisible({ timeout: 3000 }).catch(() => false);
  if (hasExperience) {
    await expBtn.click();
    // Step 4: review
    await page.locator('[data-testid="step-review"]').waitFor({ state: 'visible', timeout: 10000 });
  } else {
    // No experiences configured for F1 25 — step is still rendered (not an error)
    await expect(page.locator('[data-testid="step-select-experience"]')).toBeVisible();
  }
});

// BROW-02 — AC wizard: preset path reaches review through AC-specific steps
// Expected flow: select_plan → select_game → [session_splits (if tier ≥ 20min)] →
//   player_mode → session_type → ai_config → select_experience → driving_settings → review
// select_track and select_car are NOT in this path (experienceMode defaults to "preset").
// Source: useSetupWizard.ts lines 147–157 — preset mode removes select_track and select_car.
test('AC wizard: preset path navigates through AC steps and reaches review', async ({ page }) => {
  await enterWizardViaStaffWalkIn(page);

  // Step 1: select_plan — pick first available tier
  await page.locator('[data-testid^="tier-option-"]').first().click();

  // Step 2: select_game — pick Assetto Corsa
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="game-option-assetto_corsa"]').click();

  // Step 3 (conditional): session_splits — skipped if selected tier is < 20min (e.g. trial tier = 5min)
  const hasSplits = await page
    .locator('[data-testid="step-session-splits"]')
    .isVisible({ timeout: 3000 })
    .catch(() => false);
  if (hasSplits) {
    await page.locator('[data-testid^="split-option-"]').first().click();
  }

  // Step: player_mode — click single player button (text /single/i)
  await page.locator('[data-testid="step-player-mode"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.getByRole('button', { name: /single/i }).first().click();

  // Step: session_type — click first option (practice / race / trackday)
  await page.locator('[data-testid="step-session-type"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="step-session-type"] button').first().click();

  // Step: ai_config — advance with next/continue button (AI off is the default)
  await page.locator('[data-testid="step-ai-config"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="wizard-next-btn"]').click();

  // Step: select_experience — preset mode shows this instead of track/car selection
  await page.locator('[data-testid="step-select-experience"]').waitFor({ state: 'visible', timeout: 10000 });
  const acExpBtn = page.locator('[data-testid^="experience-option-"]').first();
  const hasAcExperience = await acExpBtn.isVisible({ timeout: 3000 }).catch(() => false);
  if (hasAcExperience) {
    await acExpBtn.click();
  }

  // Step: driving_settings (only in AC flow)
  const hasDrivingSettings = await page
    .locator('[data-testid="step-driving-settings"]')
    .isVisible({ timeout: 5000 })
    .catch(() => false);
  if (hasDrivingSettings) {
    await page.locator('[data-testid="step-driving-settings"] button').first().click();
  }

  // Final: review step
  await page.locator('[data-testid="step-review"]').waitFor({ state: 'visible', timeout: 10000 });

  // Preset mode must never show select_track or select_car (confirmed absent throughout)
  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
});

// BROW-04 — Staff mode walk-in bypasses OTP and reaches wizard directly
// Source: book/page.tsx — isStaffMode=true renders walkin-btn on phone screen.
// Clicking it calls handleStaffWalkIn() which sets authToken="staff-walkin" and phase="wizard".
// booking-otp-screen must never appear.
test('staff mode: walkin-btn bypasses OTP and reaches wizard', async ({ page }) => {
  await page.goto('/book?staff=true&pod=pod-8', { waitUntil: 'networkidle' });

  // Staff mode indicator must be visible on the phone screen
  await expect(page.getByText(/Staff Mode/i)).toBeVisible();

  // walkin-btn is only rendered when isStaffMode === true
  const walkinBtn = page.locator('[data-testid="walkin-btn"]');
  await expect(walkinBtn).toBeVisible();

  // Click walk-in — transitions directly to wizard without OTP
  await walkinBtn.click();

  // Wizard starts at select_plan — verify within 10s
  await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 });

  // OTP and phone screens must NOT be visible after walk-in
  await expect(page.locator('[data-testid="booking-otp-screen"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="booking-phone-screen"]')).not.toBeVisible();
});

// BROW-05 — Experience filtering: F1 25 shows no AC-specific steps
// Source: useSetupWizard.ts getFlow() — non-AC filter removes select_track, select_car,
// driving_settings from the flow entirely. The select_experience step remains and is
// rendered even if no F1 25 experiences are configured in the DB (empty list is acceptable).
test('experience filtering: F1 25 shows no AC-specific steps', async ({ page }) => {
  await enterWizardViaStaffWalkIn(page);

  // select_plan
  await page.locator('[data-testid^="tier-option-"]').first().click();

  // select_game — F1 25
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });
  await page.locator('[data-testid="game-option-f1_25"]').click();

  // select_experience must appear — even if the DB has no F1 25 experiences (empty list)
  await page.locator('[data-testid="step-select-experience"]').waitFor({ state: 'visible', timeout: 10000 });
  await expect(page.locator('[data-testid="step-select-experience"]')).toBeVisible();

  // AC-only steps must be absent from the DOM for non-AC games
  await expect(page.locator('[data-testid="step-select-track"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-select-car"]')).not.toBeVisible();
  await expect(page.locator('[data-testid="step-driving-settings"]')).not.toBeVisible();
});

// BROW-06 — UI navigation: back button returns to previous step, step title updates
// Source: useSetupWizard.ts goBack() — moves to flow[idx - 1]; wizard-back-btn testid
// added in Phase 42 to both back button instances in SetupWizard.tsx footer.
test('navigation: back button returns to previous step and step title updates', async ({ page }) => {
  await enterWizardViaStaffWalkIn(page);

  // select_plan → select first tier
  await page.locator('[data-testid^="tier-option-"]').first().click();

  // select_game → pick F1 25 (3-step non-AC path: select_game → select_experience)
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });

  // Capture step title on select_game step
  const titleOnGame = await page
    .locator('[data-testid="wizard-step-title"]')
    .textContent({ timeout: 5000 });
  expect(titleOnGame).toBeTruthy();

  await page.locator('[data-testid="game-option-f1_25"]').click();

  // Now on select_experience — capture title
  await page.locator('[data-testid="step-select-experience"]').waitFor({ state: 'visible', timeout: 10000 });
  const titleOnExperience = await page
    .locator('[data-testid="wizard-step-title"]')
    .textContent({ timeout: 5000 });
  expect(titleOnExperience).toBeTruthy();

  // Back: select_experience → select_game
  await page.locator('[data-testid="wizard-back-btn"]').click();
  await page.locator('[data-testid="step-select-game"]').waitFor({ state: 'visible', timeout: 10000 });

  const titleAfterFirstBack = await page
    .locator('[data-testid="wizard-step-title"]')
    .textContent({ timeout: 5000 });
  // Title must have changed when navigating back
  expect(titleAfterFirstBack).not.toBe(titleOnExperience);

  // Back: select_game → select_plan
  await page.locator('[data-testid="wizard-back-btn"]').click();
  await page.locator('[data-testid="step-select-plan"]').waitFor({ state: 'visible', timeout: 10000 });
});
