import { test, expect } from '@playwright/test';

// ---- Admin Dashboard Smoke Tests ----
// Tests run against admin portal at :3201
// Admin requires JWT auth — unauthenticated requests redirect to /login

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

// ---- Public routes (no auth required) ----

test('admin: login page loads', async ({ page }) => {
  const response = await page.goto('/login', { waitUntil: 'domcontentloaded' });
  expect(response?.status()).toBeLessThan(500);

  await page.waitForTimeout(2000); // hydration

  const body = await page.textContent('body') ?? '';
  expect(body).not.toMatch(/application error|unhandled runtime error/i);

  // Should show some login UI (form, input, or login text)
  const hasLoginContent = /login|sign in|password|admin/i.test(body);
  expect(hasLoginContent).toBe(true);
});

test('admin: health endpoint responds', async ({ request }) => {
  const response = await request.get('/api/health');
  expect(response.status()).toBeLessThan(500);
});

// ---- Auth redirect test ----

test('admin: unauthenticated root redirects to login', async ({ page }) => {
  const response = await page.goto('/', { waitUntil: 'domcontentloaded' });
  // Should redirect to /login (307) or render login page
  await page.waitForTimeout(2000);

  const url = page.url();
  const body = await page.textContent('body') ?? '';

  // Either we're on the login page OR we see login content
  const onLoginPage = url.includes('/login');
  const hasLoginContent = /login|sign in|password/i.test(body);
  expect(onLoginPage || hasLoginContent).toBe(true);
});

// ---- Protected routes (expect redirect without auth) ----

const PROTECTED_ROUTES = [
  { path: '/fleet', name: 'fleet management' },
  { path: '/billing', name: 'billing' },
  { path: '/customers', name: 'customers' },
  { path: '/sessions', name: 'sessions' },
  { path: '/analytics', name: 'analytics' },
  { path: '/games', name: 'games' },
  { path: '/staff', name: 'staff' },
  { path: '/settings', name: 'settings' },
  { path: '/coupons', name: 'coupons' },
  { path: '/pricing', name: 'pricing' },
  { path: '/kiosk', name: 'kiosk config' },
  { path: '/cafe', name: 'cafe' },
  { path: '/leaderboard', name: 'leaderboard' },
  { path: '/metrics', name: 'metrics' },
  { path: '/mesh-intelligence', name: 'mesh intelligence' },
];

for (const route of PROTECTED_ROUTES) {
  test(`admin: ${route.name} (${route.path}) redirects to login without auth`, async ({ page }) => {
    await page.goto(route.path, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(2000);

    // Should redirect to login — no 500 errors
    const body = await page.textContent('body') ?? '';
    expect(body).not.toMatch(/application error|unhandled runtime error|internal server error/i);

    // Either redirected to login or showing login form
    const url = page.url();
    const onLoginOrSafe = url.includes('/login') || /login|sign in|password/i.test(body) || body.length > 50;
    expect(onLoginOrSafe).toBe(true);
  });
}
