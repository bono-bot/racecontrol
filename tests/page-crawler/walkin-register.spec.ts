/**
 * E2E coverage for the new "+ Register New Walk-In" form inside the staff
 * kiosk SetupWizard register_driver step. Runs against a local dev server
 * on :3399. Bypasses the StaffLoginScreen by injecting sessionStorage
 * values; WS auth uses NEXT_PUBLIC_WS_TOKEN, not staff JWT.
 *
 * Test cases:
 *   1. Form renders + minor flow (guardian field appears)
 *   2. Validation: missing name / DOB / waiver
 *   3. Adult flow + REAL POST to /venue/register + auto-advance to select_plan
 *
 * Run: npx playwright test --config tests/page-crawler/playwright.config.ts walkin-register.spec.ts --project=kiosk
 */

import { test, expect, type BrowserContext } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const KIOSK_BASE = process.env.KIOSK_DEV_URL ?? 'http://127.0.0.1:3399';
const SCREENSHOT_DIR = path.resolve(__dirname, '../screenshots');

test.use({ baseURL: KIOSK_BASE });

async function setupAuthBypass(context: BrowserContext): Promise<string> {
  const fakeJwt =
    'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.' +
    Buffer.from(JSON.stringify({ sub: 'TEST_ONLY_E2E', exp: Math.floor(Date.now() / 1000) + 1800 })).toString('base64url') +
    '.fake';

  await context.addCookies([
    {
      name: 'kiosk_staff_jwt',
      value: fakeJwt,
      domain: '127.0.0.1',
      path: '/',
      expires: Math.floor(Date.now() / 1000) + 1800,
      httpOnly: false,
      secure: false,
      sameSite: 'Strict',
    },
  ]);

  await context.addInitScript(({ token }) => {
    try {
      sessionStorage.setItem('kiosk_staff_name', 'TEST_ONLY_E2E');
      sessionStorage.setItem('kiosk_staff_id', 'TEST_ONLY_E2E');
      sessionStorage.setItem('kiosk_staff_token', token);
    } catch {}
  }, { token: fakeJwt });

  return fakeJwt;
}

async function openWizardOnFirstPod(page: import('@playwright/test').Page) {
  await page.goto('/kiosk/staff', { waitUntil: 'domcontentloaded', timeout: 90_000 });
  await page.waitForTimeout(15_000); // WS connect + pod stream

  const podHeader = page.locator('span', { hasText: /^Pod \d+$/ }).first();
  await podHeader.waitFor({ state: 'visible', timeout: 15_000 });
  await podHeader.locator('..').locator('..').locator('..').click();

  const wizardStep = page.locator('[data-testid="step-register-driver"]');
  await wizardStep.waitFor({ state: 'visible', timeout: 15_000 });
}

test('case 1: form renders + minor flow (guardian field appears)', async ({ browser }) => {
  fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
  const context = await browser.newContext();
  await setupAuthBypass(context);
  const page = await context.newPage();
  page.on('pageerror', (err) => console.log(`[pageerror]`, err.message));

  await openWizardOnFirstPod(page);

  const toggle = page.locator('[data-testid="register-walkin-toggle"]');
  await expect(toggle).toBeVisible();
  await toggle.click();

  const form = page.locator('[data-testid="walkin-register-form"]');
  await form.waitFor({ state: 'visible', timeout: 5_000 });

  await page.locator('[data-testid="walkin-name"]').fill('TEST_ONLY_E2E Minor');
  await page.locator('[data-testid="walkin-dob"]').fill('2015-06-15');
  await page.locator('[data-testid="walkin-waiver"]').check();

  const guardian = page.locator('[data-testid="walkin-guardian"]');
  await expect(guardian).toBeVisible();
  await guardian.fill('TEST_ONLY_E2E Guardian');

  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c1-minor-form.png'), fullPage: true });

  await page.locator('[data-testid="walkin-cancel"]').click();
  await form.waitFor({ state: 'hidden', timeout: 3_000 });
  await context.close();
});

test('case 2: validation surfaces errors', async ({ browser }) => {
  fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
  const context = await browser.newContext();
  await setupAuthBypass(context);
  const page = await context.newPage();

  await openWizardOnFirstPod(page);
  await page.locator('[data-testid="register-walkin-toggle"]').click();
  const form = page.locator('[data-testid="walkin-register-form"]');
  await form.waitFor({ state: 'visible' });
  const submit = page.locator('[data-testid="walkin-submit"]');

  // 2a: empty name
  await submit.click();
  await expect(form.locator('text=/Name must be at least 2 characters/i')).toBeVisible();
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c2a-empty-name.png'), fullPage: true });

  // 2b: name OK, dob empty
  await page.locator('[data-testid="walkin-name"]').fill('TEST_ONLY_E2E NoDob');
  await submit.click();
  await expect(form.locator('text=/Date of birth is required/i')).toBeVisible();
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c2b-no-dob.png'), fullPage: true });

  // 2c: name + dob (adult), waiver unchecked
  await page.locator('[data-testid="walkin-dob"]').fill('1990-01-01');
  await submit.click();
  await expect(form.locator('text=/Waiver consent is required/i')).toBeVisible();
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c2c-no-waiver.png'), fullPage: true });

  // 2d: minor without guardian
  await page.locator('[data-testid="walkin-dob"]').fill('2015-06-15');
  await page.locator('[data-testid="walkin-waiver"]').check();
  await submit.click();
  await expect(form.locator('text=/Guardian name is required/i')).toBeVisible();
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c2d-no-guardian.png'), fullPage: true });

  await context.close();
});

test('case 3: adult flow + real POST + auto-advance to select_plan', async ({ browser }) => {
  fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
  const context = await browser.newContext();
  await setupAuthBypass(context);
  const page = await context.newPage();

  // Capture network for the venue/register call
  const venueCalls: { status: number; body: string }[] = [];
  page.on('response', async (res) => {
    if (res.url().includes('/api/v1/venue/register')) {
      try {
        venueCalls.push({ status: res.status(), body: (await res.text()).slice(0, 300) });
      } catch {}
    }
  });

  await openWizardOnFirstPod(page);
  await page.locator('[data-testid="register-walkin-toggle"]').click();
  const form = page.locator('[data-testid="walkin-register-form"]');
  await form.waitFor({ state: 'visible' });

  // Unique name to force fresh registration (avoid the duplicate path)
  const uniqueName = `TEST_ONLY_E2E Adult ${Date.now()}`;
  await page.locator('[data-testid="walkin-name"]').fill(uniqueName);
  await page.locator('[data-testid="walkin-dob"]').fill('1990-05-15');
  await page.locator('[data-testid="walkin-waiver"]').check();

  // Guardian field must NOT appear for adults
  await expect(page.locator('[data-testid="walkin-guardian"]')).toHaveCount(0);

  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c3-adult-form.png'), fullPage: true });

  await page.locator('[data-testid="walkin-submit"]').click();

  // Form should disappear (closed by handleSelectDriver -> resetRegisterForm + goNext)
  await form.waitFor({ state: 'hidden', timeout: 8_000 });

  // Wizard should now be on select_plan step (handleSelectDriver calls goNext())
  const planStep = page.locator('[data-testid="step-select-plan"]');
  await planStep.waitFor({ state: 'visible', timeout: 8_000 });

  // Header should show the new driver's name
  const headerName = page.locator('text=/' + uniqueName.slice(0, 24) + '/');
  await expect(headerName).toBeVisible();

  await page.screenshot({ path: path.join(SCREENSHOT_DIR, 'walkin-c3-after-submit-plan-step.png'), fullPage: true });

  // Confirm at least one POST hit /venue/register and returned 2xx with driver_id
  expect(venueCalls.length, 'at least one /venue/register call').toBeGreaterThan(0);
  const last = venueCalls[venueCalls.length - 1];
  expect(last.status, 'venue/register HTTP status').toBe(200);
  expect(last.body, 'response body').toMatch(/driver_id/);

  console.log('[venue/register response]', last.body);
  await context.close();
});
