/**
 * Shared Playwright mocks for kiosk E2E tests.
 *
 * The kiosk calls racecontrol's REST API at build-time-baked
 * `NEXT_PUBLIC_API_URL` (in CI: http://localhost:8080, which is not served
 * by anything). Without mocks, every fetch fails, the staff login flow hangs,
 * and wizard tests time out.
 *
 * Usage pattern:
 *
 *   import { setupKioskApiMocks, loginAndOpenWizard } from './helpers/mocks';
 *
 *   test.beforeEach(async ({ page }) => {
 *     await setupKioskApiMocks(page);
 *   });
 *
 *   test('my test', async ({ page }) => {
 *     await loginAndOpenWizard(page);
 *     // ...
 *   });
 *
 * Spec files may override any individual route AFTER `setupKioskApiMocks`
 * by calling `page.route()` again — Playwright uses last-registered-wins.
 */

import type { Page } from '@playwright/test';

// ── Test fixtures (snake_case — mirrors rc-common Rust structs) ──────────────

export const MOCK_STAFF_AUTH = {
  status: 'ok',
  staff_id: 'staff-e2e-001',
  staff_name: 'E2E Test Staff',
  token: 'test-staff-jwt-token',
};

export const MOCK_PODS_IDLE = {
  pods: Array.from({ length: 8 }, (_, i) => ({
    id: String(i + 1),
    pod_number: i + 1,
    name: `Pod ${i + 1}`,
    status: 'idle',
    game_state: 'idle',
    driving_state: 'idle',
    ws_connected: true,
  })),
};

export const MOCK_INVENTORY_OK = {
  pod_number: 1,
  pod_name: 'Pod 1',
  sim: 'assetto_corsa',
  games: [
    { key: 'assetto_corsa', display_name: 'Assetto Corsa', installed: true,
      cars: ['bmw_m3', 'ferrari_458'], tracks: ['spa', 'monza'] },
    { key: 'f1_25', display_name: 'F1 25', installed: true, cars: [], tracks: [] },
    { key: 'forza_horizon_5', display_name: 'Forza Horizon 5', installed: true, cars: [], tracks: [] },
    { key: 'iracing', display_name: 'iRacing', installed: true, cars: [], tracks: [] },
    { key: 'le_mans_ultimate', display_name: 'Le Mans Ultimate', installed: true, cars: [], tracks: [] },
  ],
  ai_count_range: { min: 1, max: 7 },
  fetched_at: '10:00 IST',
};

export const MOCK_EXPERIENCES = {
  experiences: [
    { id: 'exp-1', name: 'Spa Classic', game: 'assetto_corsa', track: 'spa',
      car: 'bmw_m3', duration_minutes: 30, start_type: 'hotlap',
      ac_preset_id: 'preset-spa-bmw', sort_order: 1, is_active: true },
    { id: 'exp-2', name: 'Monza Sprint', game: 'assetto_corsa', track: 'monza',
      car: 'ferrari_458', duration_minutes: 20, start_type: 'race',
      ac_preset_id: 'preset-monza-ferrari', sort_order: 2, is_active: true },
  ],
};

export const MOCK_DRIVERS = {
  drivers: [
    { id: 'drv-1', name: 'E2E Test Driver', phone: '9999999999' },
    { id: 'drv-2', name: 'Test Driver', phone: '9999999998' },
  ],
};

export const MOCK_PRICING = {
  tiers: [
    { id: 'tier-trial', name: 'Free Trial', duration_minutes: 5,
      price_paise: 0, is_active: true, is_trial: true },
    { id: 'tier-30', name: '30 Min', duration_minutes: 30,
      price_paise: 70000, is_active: true, is_trial: false },
    { id: 'tier-60', name: '60 Min', duration_minutes: 60,
      price_paise: 90000, is_active: true, is_trial: false },
  ],
};

export const MOCK_AC_CATALOG = {
  tracks: {
    featured: [
      { id: 'spa', name: 'Spa-Francorchamps', category: 'Circuit', country: 'Belgium' },
      { id: 'monza', name: 'Monza', category: 'Circuit', country: 'Italy' },
      { id: 'nurburgring', name: 'Nurburgring', category: 'Circuit', country: 'Germany' },
    ],
    all: [
      { id: 'spa', name: 'Spa-Francorchamps', category: 'Circuit', country: 'Belgium' },
      { id: 'monza', name: 'Monza', category: 'Circuit', country: 'Italy' },
      { id: 'nurburgring', name: 'Nurburgring', category: 'Circuit', country: 'Germany' },
    ],
  },
  cars: {
    featured: [
      { id: 'bmw_m3', name: 'BMW M3 E30', category: 'Touring' },
      { id: 'ferrari_458', name: 'Ferrari 458', category: 'GT' },
      { id: 'lamborghini_huracan', name: 'Lamborghini Huracan', category: 'GT' },
    ],
    all: [
      { id: 'bmw_m3', name: 'BMW M3 E30', category: 'Touring' },
      { id: 'ferrari_458', name: 'Ferrari 458', category: 'GT' },
      { id: 'lamborghini_huracan', name: 'Lamborghini Huracan', category: 'GT' },
    ],
  },
  categories: { tracks: ['Circuit', 'Hillclimb'], cars: ['Touring', 'GT', 'Formula'] },
  presets: [],
};

export const MOCK_GAMES_CATALOG = {
  games: [
    { id: 'assetto_corsa', name: 'Assetto Corsa', abbr: 'AC' },
    { id: 'f1_25', name: 'F1 25', abbr: 'F1' },
    { id: 'iracing', name: 'iRacing', abbr: 'iR' },
    { id: 'le_mans_ultimate', name: 'Le Mans Ultimate', abbr: 'LMU' },
    { id: 'forza_horizon_5', name: 'Forza Horizon 5', abbr: 'FH5' },
  ],
};

// ── Mock setup ───────────────────────────────────────────────────────────────

/**
 * Register the full kiosk backend mock set on a Playwright page.
 *
 * Covers: staff auth, pods, fleet health, experiences, drivers, pricing,
 * game catalog, AC catalog, per-pod inventory, launch stub. Any later
 * `page.route()` call overrides these for specific test needs.
 */
export async function setupKioskApiMocks(page: Page): Promise<void> {
  await page.route('**/api/v1/staff/validate-pin', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_STAFF_AUTH) }));

  await page.route('**/api/v1/pods', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_PODS_IDLE) }));

  await page.route('**/api/v1/fleet/health', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify({ pods: [] }) }));

  await page.route('**/api/v1/kiosk/experiences', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_EXPERIENCES) }));

  await page.route('**/api/v1/drivers*', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_DRIVERS) }));

  await page.route('**/api/v1/pricing', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_PRICING) }));

  await page.route('**/api/v1/customer/ac/catalog', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_AC_CATALOG) }));

  await page.route('**/api/v1/games/catalog', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_GAMES_CATALOG) }));

  await page.route('**/api/v1/pods/*/inventory', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify(MOCK_INVENTORY_OK) }));

  // Launch stub — default to success; tests that want errors override
  await page.route('**/api/v1/games/launch', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify({ ok: true, session_id: 'e2e-session-001' }) }));

  await page.route('**/api/v1/app-health', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json',
      body: JSON.stringify({ healthy: true, version: 'e2e-test' }) }));
}

/**
 * Go to /kiosk/staff, handle the PIN flow if the app asks for it.
 * Assumes `setupKioskApiMocks` has already run (or matching mocks are in place).
 */
export async function loginAsStaff(page: Page): Promise<void> {
  await page.goto('/kiosk/staff', { waitUntil: 'domcontentloaded' });
  await page.waitForTimeout(1500);

  const signInBtn = page.getByText('Tap to Sign In');
  if (await signInBtn.isVisible({ timeout: 4000 }).catch(() => false)) {
    await signInBtn.click();
    await page.waitForTimeout(400);
    const pin = (process.env.STAFF_PIN ?? '7080').split('');
    for (const digit of pin) {
      await page.locator(`button:has-text("${digit}")`).first().click();
      await page.waitForTimeout(80);
    }
    await page.waitForTimeout(1500);
  }
}

/**
 * Click a pod card in the staff dashboard (defaults to Pod 1).
 */
export async function openPodWizard(page: Page, podNumber = 1): Promise<void> {
  const podCard = page.getByText(`Pod ${podNumber}`).first();
  if (await podCard.isVisible({ timeout: 5000 }).catch(() => false)) {
    await podCard.click();
    await page.waitForTimeout(1000);
  }
}

/**
 * Convenience: login + open Pod 1 wizard in one call (matches the
 * pre-helper inline pattern in setup-wizard-inventory.spec.ts).
 */
export async function loginAndOpenWizard(page: Page, podNumber = 1): Promise<void> {
  await loginAsStaff(page);
  await openPodWizard(page, podNumber);
}
