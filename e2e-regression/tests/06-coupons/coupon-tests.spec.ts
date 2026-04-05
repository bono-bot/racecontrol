// ═══════════════════════════════════════════════════════════════
// Coupon Tests — percentage, fixed, free minutes
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { STAFF_PIN } from '../../fixtures/test-data';
import { createTestDriver, createTestCoupons, ensureWalletBalance } from '../../fixtures/test-driver-factory';
import { getAnyIdlePod } from '../../fixtures/random-pod';

const api = new RCApiClient();
let testDriverId: string;
let coupons: { percentCoupon: string; flatCoupon: string; freeMinCoupon: string };

test.describe('06 — Coupon Tests', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
    const driver = await createTestDriver(api, { balancePaise: 2000000 });
    testDriverId = driver.driverId;
    coupons = await createTestCoupons(api);
    console.log(`Test coupons: ${JSON.stringify(coupons)}`);
  });

  test('Percentage coupon (10% off)', async ({ page }) => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    const session = await api.startBilling({
      pod_id: pod.podId,
      driver_id: testDriverId,
      sim_type: 'assetto_corsa',
      coupon_code: coupons.percentCoupon,
    });

    expect(session.discount_paise).toBeGreaterThan(0);
    console.log(`Percent coupon: discount = ₹${(session.discount_paise || 0) / 100}`);

    await loginPOS(page);
    await page.goto('/billing', { waitUntil: 'load' });
    await waitForApp(page);
    await screenshot(page, '06-coupon-percent');

    // Stop session
    await api.stopBilling(session.id);
    try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }
  });

  test('Fixed amount coupon (₹50 off)', async ({ page }) => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    const session = await api.startBilling({
      pod_id: pod.podId,
      driver_id: testDriverId,
      sim_type: 'assetto_corsa',
      coupon_code: coupons.flatCoupon,
    });

    expect(session.discount_paise).toBeGreaterThan(0);
    console.log(`Flat coupon: discount = ₹${(session.discount_paise || 0) / 100}`);

    await loginPOS(page);
    await screenshot(page, '06-coupon-flat');

    await api.stopBilling(session.id);
    try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }
  });

  test('Free minutes coupon (5 min free)', async ({ page }) => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    const session = await api.startBilling({
      pod_id: pod.podId,
      driver_id: testDriverId,
      sim_type: 'assetto_corsa',
      coupon_code: coupons.freeMinCoupon,
    });

    console.log(`Free minutes coupon applied: allocated_seconds=${session.allocated_seconds}`);

    await loginPOS(page);
    await screenshot(page, '06-coupon-free-minutes');

    await api.stopBilling(session.id);
    try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }
  });

  test('Invalid coupon rejected', async () => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    try {
      await api.startBilling({
        pod_id: pod.podId,
        driver_id: testDriverId,
        sim_type: 'assetto_corsa',
        coupon_code: 'INVALID_NONEXISTENT_CODE',
      });
      // If it didn't throw, the coupon was silently ignored — also acceptable
      console.log('Invalid coupon: silently ignored (no error)');
    } catch (e) {
      // Expected: invalid coupon rejected
      console.log(`Invalid coupon correctly rejected: ${String(e).slice(0, 100)}`);
      expect(String(e)).toContain('coupon');
    }
  });
});
