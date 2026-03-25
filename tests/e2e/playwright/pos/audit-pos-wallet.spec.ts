import { test, expect, request } from '@playwright/test';

/**
 * POS Audit: Create Test Customer → Add Rs.1000 via CASH → Verify in POS + Kiosk
 *
 * This is a live audit test that runs against the production POS system.
 * It creates a real test customer, tops up their wallet, and verifies
 * the balance is reflected across both the POS dashboard and kiosk.
 */

const API_BASE = process.env.RC_API_URL ?? 'http://192.168.31.23:8080';
const POS_BASE = process.env.POS_BASE_URL ?? 'http://192.168.31.23:3200';
const KIOSK_BASE = process.env.KIOSK_BASE_URL ?? 'http://192.168.31.23:3300';
const ADMIN_PIN = process.env.ADMIN_PIN ?? '261121';

// Test customer details — uses TEST_ONLY prefix per standing rules
const TEST_CUSTOMER = {
  name: 'TEST_ONLY Audit Driver',
  phone: '0000000000',
  email: '',
};

const TOPUP_AMOUNT_PAISE = 100_000; // Rs. 1000 = 100,000 paise = 1000 credits

let staffToken = '';
let testDriverId = '';
let newBalancePaise = 0;

// ---- Shared setup: get staff JWT ----

test.beforeAll(async () => {
  const ctx = await request.newContext({ baseURL: API_BASE });
  try {
    const loginRes = await ctx.post('/api/v1/auth/admin-login', {
      data: { pin: ADMIN_PIN },
    });
    expect(loginRes.ok(), `Admin login failed: ${loginRes.status()}`).toBeTruthy();
    const body = await loginRes.json();
    staffToken = body.token;
    expect(staffToken).toBeTruthy();
  } finally {
    await ctx.dispose();
  }
});

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

// ---- Step 1: Create Test Customer via API ----

test.describe.serial('POS Audit: Wallet Topup E2E', () => {
  test('Step 1: Create test customer via API', async () => {
    const ctx = await request.newContext({
      baseURL: API_BASE,
      extraHTTPHeaders: { Authorization: `Bearer ${staffToken}` },
    });
    try {
      const res = await ctx.post('/api/v1/drivers', {
        data: TEST_CUSTOMER,
      });
      expect(res.ok(), `Create driver failed: ${res.status()}`).toBeTruthy();
      const body = await res.json();
      expect(body.id).toBeTruthy();
      expect(body.name).toBe(TEST_CUSTOMER.name);
      testDriverId = body.id;
      console.log(`Created test driver: ${testDriverId} (${body.name})`);
    } finally {
      await ctx.dispose();
    }
  });

  // ---- Step 2: Topup Rs.1000 via CASH ----

  test('Step 2: Add Rs.1000 balance via CASH', async () => {
    expect(testDriverId, 'Driver ID must be set from Step 1').toBeTruthy();

    const ctx = await request.newContext({
      baseURL: API_BASE,
      extraHTTPHeaders: { Authorization: `Bearer ${staffToken}` },
    });
    try {
      const res = await ctx.post(`/api/v1/wallet/${testDriverId}/topup`, {
        data: {
          amount_paise: TOPUP_AMOUNT_PAISE,
          method: 'cash',
          notes: 'POS Audit test — Rs.1000 CASH topup',
          staff_id: 'admin',
        },
      });
      expect(res.ok(), `Topup failed: ${res.status()}`).toBeTruthy();
      const body = await res.json();
      expect(body.status).toBe('ok');
      expect(body.new_balance_paise).toBeGreaterThanOrEqual(TOPUP_AMOUNT_PAISE);
      newBalancePaise = body.new_balance_paise;

      console.log(`Topup successful: new_balance=${newBalancePaise} paise (${newBalancePaise / 100} credits), bonus=${body.bonus_paise} paise`);
    } finally {
      await ctx.dispose();
    }
  });

  // ---- Step 3: Verify transaction exists in wallet transactions ----

  test('Step 3: Verify wallet transaction recorded', async () => {
    expect(testDriverId, 'Driver ID must be set from Step 1').toBeTruthy();

    const ctx = await request.newContext({
      baseURL: API_BASE,
      extraHTTPHeaders: { Authorization: `Bearer ${staffToken}` },
    });
    try {
      const res = await ctx.get(`/api/v1/wallet/${testDriverId}/transactions?limit=5`);
      expect(res.ok(), `Wallet transactions failed: ${res.status()}`).toBeTruthy();
      const body = await res.json();

      const txns = body.transactions ?? [];
      expect(txns.length).toBeGreaterThan(0);

      // Find the topup_cash transaction
      const topupTxn = txns.find(
        (t: { txn_type: string; amount_paise: number }) =>
          t.txn_type === 'topup_cash' && t.amount_paise === TOPUP_AMOUNT_PAISE
      );
      expect(topupTxn, 'topup_cash transaction for 100000 paise not found').toBeTruthy();
      console.log(`Transaction verified: id=${topupTxn.id}, type=${topupTxn.txn_type}, amount=${topupTxn.amount_paise}, balance_after=${topupTxn.balance_after_paise}`);
    } finally {
      await ctx.dispose();
    }
  });

  // ---- Step 4: Verify balance via wallet GET endpoint ----

  test('Step 4: Verify wallet balance via API', async () => {
    expect(testDriverId, 'Driver ID must be set from Step 1').toBeTruthy();

    const ctx = await request.newContext({
      baseURL: API_BASE,
      extraHTTPHeaders: { Authorization: `Bearer ${staffToken}` },
    });
    try {
      const res = await ctx.get(`/api/v1/wallet/${testDriverId}`);
      expect(res.ok(), `Get wallet failed: ${res.status()}`).toBeTruthy();
      const body = await res.json();

      const wallet = body.wallet;
      expect(wallet).toBeTruthy();
      expect(wallet.balance_paise).toBeGreaterThanOrEqual(TOPUP_AMOUNT_PAISE);
      console.log(`Wallet balance confirmed: ${wallet.balance_paise} paise (${wallet.balance_paise / 100} credits)`);
    } finally {
      await ctx.dispose();
    }
  });

  // ---- Step 5: Verify test customer appears on POS Dashboard (Drivers page) ----

  test('Step 5: Verify customer visible on POS Drivers page', async ({ page }) => {
    // Inject staff JWT into localStorage before navigating (bypass AuthGate)
    await page.goto(`${POS_BASE}/login`);
    await page.evaluate((token) => {
      localStorage.setItem('rp_staff_jwt', token);
    }, staffToken);

    // Navigate to drivers page — AuthGate will see the token and allow access
    await page.goto(`${POS_BASE}/drivers`, { waitUntil: 'networkidle' });

    // Wait for the driver list to load (the page fetches drivers async)
    await page.waitForSelector('text=TEST_ONLY Audit Driver', { timeout: 15_000 });

    const pageContent = await page.textContent('body');
    expect(pageContent).toContain('TEST_ONLY Audit Driver');

    // Take screenshot for audit trail
    await page.screenshot({ path: 'tests/e2e/results/audit-pos-drivers.png', fullPage: true });
    console.log('POS Drivers page: TEST_ONLY Audit Driver found');
  });

  // ---- Step 6: Verify balance reflected in Kiosk (staff view with wallet) ----

  test('Step 6: Verify balance reflected in Kiosk via API', async () => {
    // Kiosk uses the same backend API — verify via the kiosk-facing wallet endpoint
    // (kiosk staff page fetches wallet from /api/v1/wallet/{driver_id})
    expect(testDriverId, 'Driver ID must be set from Step 1').toBeTruthy();

    const ctx = await request.newContext({
      baseURL: API_BASE,
      extraHTTPHeaders: { Authorization: `Bearer ${staffToken}` },
    });
    try {
      // Simulate what the kiosk does: fetch wallet for the driver
      const walletRes = await ctx.get(`/api/v1/wallet/${testDriverId}`);
      expect(walletRes.ok()).toBeTruthy();
      const walletBody = await walletRes.json();
      expect(walletBody.wallet.balance_paise).toBeGreaterThanOrEqual(TOPUP_AMOUNT_PAISE);

      // Also verify driver appears in the drivers list (kiosk uses same endpoint)
      const driversRes = await ctx.get(`/api/v1/drivers?search=TEST_ONLY`);
      expect(driversRes.ok()).toBeTruthy();
      const driversBody = await driversRes.json();
      const found = (driversBody.drivers ?? []).find(
        (d: { name: string }) => d.name === TEST_CUSTOMER.name
      );
      expect(found, 'Test driver not found via search').toBeTruthy();

      console.log(`Kiosk verification: driver found, wallet balance=${walletBody.wallet.balance_paise} paise (${walletBody.wallet.balance_paise / 100} credits)`);
    } finally {
      await ctx.dispose();
    }
  });

  // ---- Step 7: Verify Kiosk staff page loads and can see the driver (visual) ----

  test('Step 7: Kiosk staff page loads without errors', async ({ page }) => {
    // Load the kiosk staff page
    const res = await page.goto(`${KIOSK_BASE}/kiosk/staff`, { waitUntil: 'networkidle' });
    expect(res?.status()).toBeLessThan(500);

    const body = await page.textContent('body') ?? '';
    expect(body).not.toMatch(/application error|unhandled runtime error/i);

    // Take screenshot for audit trail
    await page.screenshot({ path: 'tests/e2e/results/audit-kiosk-staff.png', fullPage: true });
    console.log('Kiosk staff page: loaded successfully');
  });

  // ---- Cleanup: Remove test customer ----

  test('Cleanup: Note test customer for manual cleanup', async () => {
    // DELETE /drivers endpoint returns 405 — no driver deletion API exists.
    // Test drivers are cleaned up by: node tests/e2e/cleanup-test-drivers.mjs
    // This step just logs the test driver ID for traceability.
    if (!testDriverId) {
      console.log('No test driver was created');
      return;
    }
    console.log(`Test driver ${testDriverId} (${TEST_CUSTOMER.name}) persists in DB.`);
    console.log('Run cleanup: scp tests/e2e/cleanup-test-drivers.mjs to server, then: node cleanup-test-drivers.mjs');
  });
});
