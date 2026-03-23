import { test, expect } from '../fixtures/cleanup';

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

// ---- Kiosk: Every page loads without errors ----

const KIOSK_ROUTES = [
  { path: '/', name: 'customer landing' },
  { path: '/book', name: 'booking page' },
  { path: '/staff', name: 'staff login' },
  { path: '/settings', name: 'settings' },
  { path: '/spectator', name: 'spectator view' },
  { path: '/debug', name: 'debug panel' },
  { path: '/control', name: 'control panel' },
  { path: '/fleet', name: 'fleet overview' },
  { path: '/pod/8', name: 'pod 8 kiosk view' },
];

for (const route of KIOSK_ROUTES) {
  test(`kiosk smoke: ${route.name} (${route.path}) loads`, async ({ page }) => {
    const response = await page.goto(route.path, { waitUntil: 'networkidle' });
    expect(response?.status()).toBeLessThan(500);

    const body = await page.textContent('body') ?? '';
    expect(body).not.toMatch(/application error|unhandled runtime error|a client-side exception/i);
  });
}
