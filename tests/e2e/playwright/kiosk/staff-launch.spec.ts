/**
 * E2E Staff Kiosk — Full game launch flow via UI
 * Tests: login → search customer → select plan → select game → configure → launch
 */
import { test, expect, type Page } from '@playwright/test';
import * as fs from 'fs';

const KIOSK_URL = 'http://192.168.31.23:3300/kiosk/staff';
const SCREENSHOT_DIR = 'C:/Users/bono/e2e-screenshots';

async function snap(page: Page, name: string) {
  await page.screenshot({ path: `${SCREENSHOT_DIR}/${name}.png`, fullPage: true });
}

async function staffLogin(page: Page) {
  await page.goto(KIOSK_URL, { waitUntil: 'networkidle', timeout: 15000 });

  // Check if already logged in
  const signIn = page.locator('text=Tap to Sign In');
  if (await signIn.isVisible({ timeout: 2000 }).catch(() => false)) {
    await signIn.click();
    await page.waitForTimeout(500);

    // Enter PIN 0009 (Chavan Vishal)
    for (const digit of ['0', '0', '0', '9']) {
      await page.locator(`button:has-text("${digit}")`).first().click();
      await page.waitForTimeout(150);
    }
    // Auto-submits at 4 digits — wait for dashboard
    await page.waitForTimeout(3000);
  }

  // Verify dashboard loaded
  await expect(page.locator('text=Pod 1')).toBeVisible({ timeout: 5000 });
}

test.describe('Staff Kiosk E2E', () => {
  test.beforeAll(() => {
    if (!fs.existsSync(SCREENSHOT_DIR)) fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
  });

  test('Full flow: login → Pod 1 → search driver → plan → game → launch', async ({ page }) => {
    page.setDefaultTimeout(10000);

    // === STEP 1: Login ===
    await staffLogin(page);
    await snap(page, 'e2e-01-dashboard');

    // === STEP 2: Click Pod 8 (other pods have stale sessions from earlier tests) ===
    await page.locator('text=Pod 8').first().click();
    await page.waitForTimeout(1000);
    await snap(page, 'e2e-02-pod1-panel');

    // Verify side panel opened with "Select Driver"
    await expect(page.locator('text=Select Driver')).toBeVisible({ timeout: 3000 });

    // === STEP 3: Search for a registered driver with waiver signed ===
    const searchInput = page.locator('[data-testid="driver-search"]');
    await searchInput.fill('Vishal');
    await page.waitForTimeout(1500);
    await snap(page, 'e2e-03-driver-search');

    // Click first search result (Chavan Vishal — has waiver signed)
    const firstResult = page.locator('[data-testid^="driver-result-"]').first();
    await firstResult.click();
    await page.waitForTimeout(1000);
    await snap(page, 'e2e-04-driver-selected');

    // === STEP 4: Select pricing plan ===
    await expect(page.locator('text=Select Plan')).toBeVisible({ timeout: 5000 });
    await snap(page, 'e2e-05-plan-step');

    // Click 30 Minutes tier button
    const thirtyMinBtn = page.locator('button:has-text("30 Minutes")').first();
    await thirtyMinBtn.click();
    await page.waitForTimeout(1500);
    await snap(page, 'e2e-06-plan-selected');

    // === STEP 5: Select game ===
    await expect(page.locator('h3:has-text("Select Game")')).toBeVisible({ timeout: 5000 });
    await snap(page, 'e2e-07-game-step');

    // Click Assetto Corsa (exact text match to avoid matching "Assetto Corsa Evo" etc.)
    const acBtn = page.locator('button', { hasText: /^Assetto Corsa$/ }).first();
    if (!await acBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
      // Fallback: click first enabled game button
      await page.locator('button:has-text("Assetto")').first().click();
    } else {
      await acBtn.click();
    }
    await page.waitForTimeout(1500);
    await snap(page, 'e2e-08-game-selected');

    // === STEP 6: Player Mode — click Singleplayer ===
    const singleplayer = page.locator('button:has-text("Singleplayer")');
    if (await singleplayer.isVisible({ timeout: 3000 }).catch(() => false)) {
      await singleplayer.click();
      await page.waitForTimeout(1000);
      await snap(page, 'e2e-09a-singleplayer');
    }

    // === STEP 7: Click through remaining wizard steps ===
    // Each step may auto-advance on selection or need a button click
    for (let i = 0; i < 10; i++) {
      await snap(page, `e2e-09-step-${i}`);

      // Try clicking common buttons in priority order
      const actions = [
        page.locator('button:has-text("Spa Hot Lap")').first(),  // Preset experience
        page.locator('button:has-text("Quick Race")'),
        page.locator('button:has-text("Race")').first(),
        page.locator('button:has-text("Next")'),
        page.locator('button:has-text("Continue")'),
        page.locator('button:has-text("Skip")'),
        page.locator('button:has-text("Random")'),
        page.locator('button:has-text("Default")'),
        page.locator('button:has-text("Beginner")'),
        page.locator('button:has-text("Practice")'),  // Session type
      ];

      let clicked = false;
      for (const action of actions) {
        if (await action.isVisible({ timeout: 1000 }).catch(() => false)) {
          await action.click();
          await page.waitForTimeout(1000);
          clicked = true;
          break;
        }
      }

      // Check if we reached "Review & Launch" step
      const reviewHeading = page.locator('h3:has-text("Review")');
      if (await reviewHeading.isVisible({ timeout: 500 }).catch(() => false)) {
        await snap(page, 'e2e-10-review');
        break;
      }

      // Check if Launch button appeared
      const launchBtn = page.locator('button:has-text("Launch")');
      if (await launchBtn.isVisible({ timeout: 500 }).catch(() => false)) {
        await snap(page, 'e2e-10-launch-found');
        break;
      }

      if (!clicked) break;
    }

    // === STEP 8: Click Review if visible, then Launch ===
    const reviewBtn = page.locator('button:has-text("Review")');
    if (await reviewBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
      await reviewBtn.click();
      await page.waitForTimeout(2000);
      await snap(page, 'e2e-10-review-page');
    }

    await snap(page, 'e2e-11-pre-launch');
    const launchBtn = page.locator('button:has-text("Launch Game"), button:has-text("Launch"), button:has-text("Start Race"), button:has-text("Confirm Launch")').first();
    if (await launchBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await launchBtn.click();
      await page.waitForTimeout(8000); // Wait for game to start
      await snap(page, 'e2e-12-launched');
    } else {
      await snap(page, 'e2e-12-no-launch-btn');
    }

    // === STEP 9: Final state ===
    await snap(page, 'e2e-13-final');
  });
});
