import { test, expect } from '@playwright/test';

// ---- JS error capture ----
let jsErrors: string[] = [];
test.beforeEach(async ({ page }) => {
  jsErrors = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
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

// ---- POS Dashboard: Every page loads without errors ----

const POS_ROUTES = [
  { path: '/', name: 'home / pod grid' },
  { path: '/login', name: 'login' },
  { path: '/pods', name: 'pods management' },
  { path: '/drivers', name: 'drivers list' },
  { path: '/sessions', name: 'sessions' },
  { path: '/billing', name: 'billing active' },
  { path: '/billing/history', name: 'billing history' },
  { path: '/billing/pricing', name: 'pricing tiers' },
  { path: '/bookings', name: 'bookings' },
  { path: '/games', name: 'games catalog' },
  { path: '/events', name: 'events' },
  { path: '/leaderboards', name: 'leaderboards' },
  { path: '/telemetry', name: 'telemetry' },
  { path: '/ac-lan', name: 'AC LAN sessions' },
  { path: '/ac-sessions', name: 'AC session list' },
  { path: '/settings', name: 'settings' },
  { path: '/ai', name: 'AI assistant' },
  { path: '/kiosk', name: 'kiosk config' },
  { path: '/presenter', name: 'presenter / spectator' },
  { path: '/cameras', name: 'cameras live' },
  { path: '/cameras/playback', name: 'cameras playback' },
  { path: '/cafe', name: 'cafe management' },
];

for (const route of POS_ROUTES) {
  test(`POS smoke: ${route.name} (${route.path}) loads`, async ({ page }) => {
    const response = await page.goto(route.path, { waitUntil: 'networkidle' });
    expect(response?.status()).toBeLessThan(500);

    const body = await page.textContent('body') ?? '';
    expect(body).not.toMatch(/application error|unhandled runtime error|a client-side exception/i);

    // Screenshot is auto-captured by config (screenshot: 'on')
  });
}
