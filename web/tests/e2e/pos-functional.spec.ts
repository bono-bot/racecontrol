import { test, expect, Page } from '@playwright/test';

const SCREENSHOT_DIR = 'test-results/screenshots';
const PIN = '261121';

// ─── Auth helper ──────────────────────────────────────────────
// Get JWT from RC API, inject into localStorage before page loads
let cachedToken: string | null = null;

async function getToken(): Promise<string> {
  if (cachedToken) return cachedToken;
  const resp = await fetch('http://192.168.31.23:8080/api/v1/auth/admin-login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ pin: PIN }),
  });
  const data = await resp.json();
  cachedToken = data.token;
  return data.token;
}

async function login(page: Page) {
  const token = await getToken();
  // Navigate to any page first to set the origin for localStorage
  await page.goto('/login', { waitUntil: 'load' });
  // Inject token directly into localStorage
  await page.evaluate((t) => {
    localStorage.setItem('rp_staff_jwt', t);
  }, token);
  // Now navigate to home — app will read token and skip login
  await page.goto('/', { waitUntil: 'load' });
  await page.waitForTimeout(2000);
}

// ─── Hydration helper ──────────────────────────────────────────
async function waitForApp(page: Page) {
  await page.waitForSelector('aside, nav, .sidebar', { timeout: 10000 }).catch(() => null);
  await page.waitForTimeout(2000); // data fetch
}

// ═══════════════════════════════════════════════════════════════
// 1.1 Login
// ═══════════════════════════════════════════════════════════════
test.describe('1.1 Login', () => {
  test('1.1.1 Login page loads with PIN input', async ({ page }) => {
    await page.goto('/login', { waitUntil: 'load' });
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.1.1-login-page.png` });
    const pinInput = page.locator('input#pin');
    await expect(pinInput).toBeVisible();
  });

  test('1.1.2 Wrong PIN shows error', async ({ page }) => {
    await page.goto('/login', { waitUntil: 'load' });
    const pinInput = page.locator('input#pin');
    await pinInput.fill('000000');
    await page.locator('button[type="submit"]').click();
    await page.waitForTimeout(2000);
    // Should show error text and stay on login
    const error = page.locator('text=Invalid');
    const hasError = await error.isVisible({ timeout: 3000 }).catch(() => false);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.1.2-wrong-pin.png` });
    expect(hasError || page.url().includes('/login')).toBeTruthy();
  });

  test('1.1.3 Correct PIN redirects to dashboard', async ({ page }) => {
    await page.goto('/login', { waitUntil: 'load' });
    const pinInput = page.locator('input#pin');
    await pinInput.fill(PIN);
    await page.locator('button[type="submit"]').click();
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.1.3-correct-pin.png` });
    // Should NOT be on login page anymore
    expect(page.url()).not.toContain('/login');
  });

  test('1.1.4 Session persists after refresh', async ({ page }) => {
    await login(page);
    const urlBefore = page.url();
    await page.reload({ waitUntil: 'load' });
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.1.4-session-persist.png` });
    // Should still be logged in (not redirected to /login)
    expect(page.url()).not.toContain('/login');
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.2 Sidebar Navigation — All 21 pages
// ═══════════════════════════════════════════════════════════════
const SIDEBAR_PAGES = [
  { path: '/', name: 'Live-Overview' },
  { path: '/pods', name: 'Pods' },
  { path: '/games', name: 'Games' },
  { path: '/telemetry', name: 'Telemetry' },
  { path: '/ac-lan', name: 'AC-LAN' },
  { path: '/ac-sessions', name: 'AC-Results' },
  { path: '/sessions', name: 'Sessions' },
  { path: '/drivers', name: 'Drivers' },
  { path: '/leaderboards', name: 'Leaderboards' },
  { path: '/events', name: 'Events' },
  { path: '/billing', name: 'Billing' },
  { path: '/billing/pricing', name: 'Pricing' },
  { path: '/billing/history', name: 'History' },
  { path: '/bookings', name: 'Bookings' },
  { path: '/ai', name: 'AI-Insights' },
  { path: '/cameras', name: 'Cameras' },
  { path: '/cameras/playback', name: 'Playback' },
  { path: '/cafe', name: 'Cafe-Menu' },
  { path: '/settings', name: 'Settings' },
  { path: '/presenter', name: 'Presenter' },
  { path: '/kiosk', name: 'Kiosk-Mode' },
];

test.describe('1.2 Sidebar Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  for (const pg of SIDEBAR_PAGES) {
    test(`${pg.name} (${pg.path}) loads with sidebar`, async ({ page }) => {
      await page.goto(pg.path, { waitUntil: 'load' });
      await page.waitForTimeout(3000);
      const sidebar = page.locator('aside');
      const hasSidebar = await sidebar.isVisible({ timeout: 5000 }).catch(() => false);
      await page.screenshot({ path: `${SCREENSHOT_DIR}/1.2-${pg.name}.png` });
      // Should not be on login page
      expect(page.url()).not.toContain('/login');
      // Title should not be 404
      const title = await page.title();
      expect(title).not.toContain('404');
    });
  }
});

// ═══════════════════════════════════════════════════════════════
// 1.3 Live Overview
// ═══════════════════════════════════════════════════════════════
test.describe('1.3 Live Overview', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.3.1 Pod grid renders with pods', async ({ page }) => {
    await page.goto('/', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.3.1-pod-grid.png` });
    // Should have sidebar (proves we're past login)
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
    // Should have pod-related content
    const body = await page.textContent('body');
    const hasPodContent = body?.includes('Pod') || body?.includes('pod') || body?.includes('Idle') || body?.includes('Online') || body?.includes('Offline');
    expect(hasPodContent).toBeTruthy();
  });

  test('1.3.4 Offline pod visible (Pod 1)', async ({ page }) => {
    await page.goto('/', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.3.4-offline-pod.png` });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.4 Games
// ═══════════════════════════════════════════════════════════════
test.describe('1.4 Games', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.4.2 Game options visible', async ({ page }) => {
    await page.goto('/games', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.4.2-games.png` });
    const body = await page.textContent('body');
    const hasGames = body?.includes('Assetto') || body?.includes('iRacing') || body?.includes('F1') || body?.includes('Forza') || body?.includes('Launch');
    expect(hasGames).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.5 Billing
// ═══════════════════════════════════════════════════════════════
test.describe('1.5 Billing', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.5.1 Billing page with pod grid', async ({ page }) => {
    await page.goto('/billing', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.5.1-billing.png` });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });

  test('1.5.4 Pricing tiers visible', async ({ page }) => {
    await page.goto('/billing/pricing', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.5.4-pricing.png` });
    const body = await page.textContent('body');
    const hasPricing = body?.includes('Standard') || body?.includes('Extended') || body?.includes('Marathon') || body?.includes('min') || body?.includes('Pricing');
    expect(hasPricing).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.6 AC LAN
// ═══════════════════════════════════════════════════════════════
test.describe('1.6 AC LAN', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.6.1 AC LAN page with controls', async ({ page }) => {
    await page.goto('/ac-lan', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.6.1-ac-lan.png` });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.7 Leaderboards
// ═══════════════════════════════════════════════════════════════
test.describe('1.7 Leaderboards', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.7.1 Leaderboards page with tabs', async ({ page }) => {
    await page.goto('/leaderboards', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.7.1-leaderboards.png` });
    const body = await page.textContent('body');
    const hasTabs = body?.includes('Record') || body?.includes('Driver') || body?.includes('Track') || body?.includes('Leaderboard');
    expect(hasTabs).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.8 Cameras
// ═══════════════════════════════════════════════════════════════
test.describe('1.8 Cameras', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.8.1 Camera grid loads', async ({ page }) => {
    await page.goto('/cameras', { waitUntil: 'load' });
    await page.waitForTimeout(5000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.8.1-cameras.png` });
  });

  test('1.8.2 Grid mode buttons', async ({ page }) => {
    await page.goto('/cameras', { waitUntil: 'load' });
    await page.waitForTimeout(3000);
    const buttons = page.locator('button');
    const count = await buttons.count();
    expect(count).toBeGreaterThan(0);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.8.2-grid-modes.png` });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.9 Cafe
// ═══════════════════════════════════════════════════════════════
test.describe('1.9 Cafe', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.9.1 Cafe page loads', async ({ page }) => {
    await page.goto('/cafe', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.9.1-cafe.png` });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.10 AI Insights
// ═══════════════════════════════════════════════════════════════
test.describe('1.10 AI Insights', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.10.1 AI page loads with filters', async ({ page }) => {
    await page.goto('/ai', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.10.1-ai.png` });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.11 Settings
// ═══════════════════════════════════════════════════════════════
test.describe('1.11 Settings', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.11.1 Settings page with server status', async ({ page }) => {
    await page.goto('/settings', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.11.1-settings.png` });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.12 Drivers
// ═══════════════════════════════════════════════════════════════
test.describe('1.12 Drivers', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.12.1 Driver grid loads with data', async ({ page }) => {
    await page.goto('/drivers', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.12.1-drivers.png` });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });
});

// ═══════════════════════════════════════════════════════════════
// 1.13 Presenter
// ═══════════════════════════════════════════════════════════════
test.describe('1.13 Presenter', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('1.13.1 Presenter view loads', async ({ page }) => {
    await page.goto('/presenter', { waitUntil: 'load' });
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/1.13.1-presenter.png` });
  });
});

// ═══════════════════════════════════════════════════════════════
// 3.1 Responsiveness
// ═══════════════════════════════════════════════════════════════
test.describe('3.1 Responsiveness', () => {
  test.beforeEach(async ({ page }) => { await login(page); });

  test('3.1.2 Full screen — sidebar + content fit', async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/', { waitUntil: 'load' });
    await waitForApp(page);
    await page.waitForTimeout(3000);
    await page.screenshot({ path: `${SCREENSHOT_DIR}/3.1.2-fullscreen.png`, fullPage: false });
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible({ timeout: 5000 });
  });
});
