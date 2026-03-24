import { type Page } from '@playwright/test';

/**
 * Mock API responses for kiosk E2E tests running without a live backend.
 * Uses Playwright's route interception — no external mock server needed.
 * Call setupApiMocks(page) in beforeEach to enable.
 */

const MOCK_TIERS = [
  { id: "tier-30", name: "30 Minutes", duration_minutes: 30, price_paise: 70000, is_trial: false, is_active: true, sort_order: 1 },
  { id: "tier-60", name: "60 Minutes", duration_minutes: 60, price_paise: 90000, is_trial: false, is_active: true, sort_order: 2 },
  { id: "tier-trial", name: "Free Trial", duration_minutes: 5, price_paise: 0, is_trial: true, is_active: true, sort_order: 0 },
];

const MOCK_EXPERIENCES = [
  { id: "exp-1", name: "Monza Hot Lap", game: "assetto_corsa", track: "monza", car: "ferrari_488_gt3", duration_minutes: 30, start_type: "hotlap", sort_order: 1, is_active: true },
  { id: "exp-2", name: "Spa Endurance", game: "assetto_corsa", track: "spa", car: "porsche_911_gt3_r", duration_minutes: 60, start_type: "race", sort_order: 2, is_active: true },
  { id: "exp-3", name: "F1 Quick Race", game: "f1_25", track: "silverstone", car: "f1_2025", duration_minutes: 30, start_type: "race", sort_order: 3, is_active: true },
];

const MOCK_PODS = [
  { id: "pod-1", number: 1, name: "Pod 1", ip_address: "192.168.31.89", sim_type: "assetto_corsa", status: "idle", installed_games: ["assetto_corsa", "f1_25", "iracing"] },
  { id: "pod-2", number: 2, name: "Pod 2", ip_address: "192.168.31.33", sim_type: "assetto_corsa", status: "in_session", installed_games: ["assetto_corsa", "f1_25"] },
  { id: "pod-8", number: 8, name: "Pod 8", ip_address: "192.168.31.91", sim_type: "assetto_corsa", status: "idle", installed_games: ["assetto_corsa", "f1_25", "iracing", "le_mans_ultimate"] },
];

const MOCK_GAMES_CATALOG = [
  { id: "assetto_corsa", name: "Assetto Corsa", abbr: "AC", installed_pod_count: 8 },
  { id: "assetto_corsa_evo", name: "Assetto Corsa Evo", abbr: "ACE", installed_pod_count: 0 },
  { id: "assetto_corsa_rally", name: "Assetto Corsa Rally", abbr: "ACR", installed_pod_count: 0 },
  { id: "iracing", name: "iRacing", abbr: "iR", installed_pod_count: 6 },
  { id: "le_mans_ultimate", name: "Le Mans Ultimate", abbr: "LMU", installed_pod_count: 4 },
  { id: "f1_25", name: "F1 25", abbr: "F1", installed_pod_count: 8 },
  { id: "forza", name: "Forza Motorsport", abbr: "FRZ", installed_pod_count: 0 },
  { id: "forza_horizon_5", name: "Forza Horizon 5", abbr: "FH5", installed_pod_count: 0 },
];

const MOCK_AC_CATALOG = {
  tracks: {
    featured: [
      { id: "monza", name: "Monza", category: "Circuit" },
      { id: "spa", name: "Spa-Francorchamps", category: "Circuit" },
      { id: "nurburgring", name: "Nurburgring Nordschleife", category: "Circuit" },
    ],
    all: [
      { id: "monza", name: "Monza", category: "Circuit" },
      { id: "spa", name: "Spa-Francorchamps", category: "Circuit" },
      { id: "nurburgring", name: "Nurburgring Nordschleife", category: "Circuit" },
      { id: "brands_hatch", name: "Brands Hatch", category: "Circuit" },
    ],
  },
  cars: {
    featured: [
      { id: "ferrari_488_gt3", name: "Ferrari 488 GT3", category: "GT3" },
      { id: "porsche_911_gt3_r", name: "Porsche 911 GT3 R", category: "GT3" },
    ],
    all: [
      { id: "ferrari_488_gt3", name: "Ferrari 488 GT3", category: "GT3" },
      { id: "porsche_911_gt3_r", name: "Porsche 911 GT3 R", category: "GT3" },
      { id: "bmw_m4_gt3", name: "BMW M4 GT3", category: "GT3" },
    ],
  },
  categories: { tracks: ["Circuit"], cars: ["GT3"] },
};

// ─── Redeem PIN mock responses ───────────────────────────────────────────────

const MOCK_REDEEM_PIN_SUCCESS = {
  pod_number: 8,
  pod_id: 'pod-8',
  driver_name: 'Test Racer',
  experience_name: 'Monza Hot Lap',
  tier_name: '30 Minutes',
  allocated_seconds: 1800,
  billing_session_id: 'deferred-test-123',
};

export const MOCK_REDEEM_PIN_ERROR = {
  error: 'Invalid PIN or reservation not found',
  remaining_attempts: 8,
  status: 'invalid_pin',
};

export const MOCK_REDEEM_PIN_LOCKOUT = {
  error: 'Too many failed attempts. Please wait 5 minutes and 0 seconds.',
  lockout_remaining_seconds: 300,
  status: 'lockout',
};

export const MOCK_REDEEM_PIN_PENDING = {
  error: 'Your booking is being processed. Please try again in a minute.',
  status: 'pending_debit',
};

export const MOCK_REDEEM_PIN_INFRA_ERROR = {
  error: 'All pods are currently in use. Please wait a moment and try again.',
  status: 'error',
};

/**
 * Override the default redeem-pin mock response for a specific test.
 * Call AFTER setupApiMocks to take priority via route.continue precedence.
 */
export async function overrideRedeemPinMock(page: Page, response: Record<string, unknown>): Promise<void> {
  await page.route('**/api/v1/kiosk/redeem-pin', async (route) => {
    if (route.request().method() === 'POST') {
      return route.fulfill({ json: response });
    }
    await route.continue();
  });
}

export async function setupApiMocks(page: Page): Promise<void> {
  // Intercept all API calls to the racecontrol backend
  await page.route('**/api/v1/**', async (route) => {
    const url = route.request().url();
    const path = new URL(url).pathname.replace(/^\/kiosk/, '').replace('/api/v1', '');
    const method = route.request().method();

    // Match known endpoints
    if (method === 'GET') {
      if (path === '/pricing') return route.fulfill({ json: { tiers: MOCK_TIERS } });
      if (path === '/kiosk/experiences') return route.fulfill({ json: { experiences: MOCK_EXPERIENCES } });
      if (path === '/pods') return route.fulfill({ json: { pods: MOCK_PODS } });
      if (path === '/games/catalog') return route.fulfill({ json: { games: MOCK_GAMES_CATALOG } });
      if (path === '/customer/ac/catalog') return route.fulfill({ json: MOCK_AC_CATALOG });
      if (path === '/health') return route.fulfill({ json: { status: 'ok', version: '0.1.0' } });
      if (path === '/kiosk/settings') return route.fulfill({ json: { settings: { venue_name: 'Racing Point', tagline: 'Test Venue' } } });
      if (path === '/fleet/health') return route.fulfill({ json: { pods: [], timestamp: new Date().toISOString() } });
      if (path === '/billing/active') return route.fulfill({ json: { sessions: [] } });
    }

    if (method === 'POST') {
      if (path === '/customer/login') return route.fulfill({ json: { status: 'otp_sent' } });
      if (path === '/customer/verify-otp') return route.fulfill({ json: { token: 'test-token', driver_id: 'drv-1', driver_name: 'Test Racer' } });
      if (path === '/customer/book') return route.fulfill({ json: { pin: 'ABC123', pod_number: 8, allocated_seconds: 1800 } });
      if (path === '/kiosk/redeem-pin') return route.fulfill({ json: MOCK_REDEEM_PIN_SUCCESS });
      if (path === '/staff/validate-pin') return route.fulfill({ json: { status: 'ok', staff_id: 'staff-1', staff_name: 'Test Staff', token: 'staff-token' } });
      if (path === '/auth/kiosk/validate-pin') return route.fulfill({ json: { status: 'ok', pod_number: 8, driver_name: 'Test Racer', allocated_seconds: 1800 } });
      if (path === '/games/launch') return route.fulfill({ json: { ok: true } });
      if (path === '/billing/start') return route.fulfill({ json: { ok: true, billing_session_id: 'bill-1' } });
    }

    // Unmatched — let it fail naturally (tests should catch unexpected API calls)
    await route.fulfill({ status: 404, json: { error: `Mock not found: ${method} ${path}` } });
  });

  // Intercept WebSocket upgrade — prevent real connection attempts
  // (Playwright can't mock WebSocket, but we prevent connection errors)
  await page.route('**/ws/**', (route) => route.abort('connectionrefused'));
}
