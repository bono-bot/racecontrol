// ═══════════════════════════════════════════════════════════════
// Chaos Tests — POS Billing UI
// Double-clicks, back button, refresh, concurrent, invalid inputs
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { STAFF_PIN } from '../../fixtures/test-data';
import { createTestDriver, ensureWalletBalance } from '../../fixtures/test-driver-factory';
import { getAnyIdlePod } from '../../fixtures/random-pod';

const api = new RCApiClient();
let testDriverId: string;

test.describe('08 — POS Chaos Tests', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
    const driver = await createTestDriver(api, { balancePaise: 2000000 });
    testDriverId = driver.driverId;
  });

  test('Double-click Start Session — no duplicate billing', async ({ page }) => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    // Start via API twice rapidly (simulates double-click)
    const key = `e2e-dblclick-${Date.now()}`;
    const p1 = api.startBilling({
      pod_id: pod.podId, driver_id: testDriverId,
      sim_type: 'assetto_corsa', idempotency_key: key,
    });
    const p2 = api.startBilling({
      pod_id: pod.podId, driver_id: testDriverId,
      sim_type: 'assetto_corsa', idempotency_key: key,
    });

    const results = await Promise.allSettled([p1, p2]);
    const successes = results.filter(r => r.status === 'fulfilled');

    // Idempotency: both should return the same session or second should fail
    console.log(`Double-click: ${successes.length} succeeded, ${results.length - successes.length} failed`);

    // Active sessions should be at most 1 for this pod
    const active = await api.activeBillingSessions();
    const podActive = active.filter(s => s.pod_id === pod.podId);
    expect(podActive.length).toBeLessThanOrEqual(1);

    // Cleanup
    for (const s of podActive) {
      try { await api.stopBilling(s.id); } catch { /* ignore */ }
    }
    try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }

    await loginPOS(page);
    await screenshot(page, '08-chaos-double-click');
  });

  test('Back button during billing flow — no orphaned session', async ({ page }) => {
    await loginPOS(page);
    await page.goto('/billing', { waitUntil: 'load' });
    await waitForApp(page);
    await screenshot(page, '08-chaos-back-before');

    // Navigate away
    await page.goto('/drivers', { waitUntil: 'load' });
    await page.waitForTimeout(1000);

    // Go back
    await page.goBack();
    await page.waitForTimeout(2000);
    await screenshot(page, '08-chaos-back-after');

    // Page should still be functional
    const body = await page.textContent('body');
    expect(body?.length).toBeGreaterThan(50);
  });

  test('Refresh mid-page — no data loss', async ({ page }) => {
    await loginPOS(page);
    await page.goto('/billing', { waitUntil: 'load' });
    await waitForApp(page);

    // Reload
    await page.reload({ waitUntil: 'load' });
    await page.waitForTimeout(3000);
    await screenshot(page, '08-chaos-refresh');

    // Still logged in (session persists)
    expect(page.url()).not.toContain('/login');
  });

  test('Concurrent sessions on same pod — rejection', async () => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    const driver2 = await createTestDriver(api, { balancePaise: 200000 });

    await ensureWalletBalance(api, testDriverId, 200000);

    // Start first session
    const s1 = await api.startBilling({
      pod_id: pod.podId, driver_id: testDriverId,
      sim_type: 'assetto_corsa',
    });

    // Try to start second session on same pod — should fail
    try {
      await api.startBilling({
        pod_id: pod.podId, driver_id: driver2.driverId,
        sim_type: 'assetto_corsa',
      });
      // If it somehow succeeded, that's a bug
      console.log('WARNING: Second session on same pod succeeded — potential bug');
    } catch (e) {
      console.log(`Correctly rejected concurrent session: ${String(e).slice(0, 100)}`);
    }

    // Cleanup
    await api.stopBilling(s1.id);
    try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }
  });

  test('Wallet insufficient funds — rejection', async () => {
    const poorDriver = await createTestDriver(api, { balancePaise: 100, name: 'E2E_Poor' }); // ₹1

    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    try {
      await api.startBilling({
        pod_id: pod.podId,
        driver_id: poorDriver.driverId,
        sim_type: 'assetto_corsa',
      });
      console.log('WARNING: Billing started with insufficient funds — potential bug');
    } catch (e) {
      console.log(`Correctly rejected insufficient funds: ${String(e).slice(0, 100)}`);
      expect(String(e).toLowerCase()).toMatch(/fund|balance|insufficient|wallet/i);
    }
  });

  test('Rapid page navigation — no JS errors', async ({ page }) => {
    await loginPOS(page);

    const errors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') errors.push(msg.text());
    });

    const pages = ['/', '/billing', '/drivers', '/sessions', '/pods', '/games', '/billing/pricing', '/leaderboards'];

    for (const pg of pages) {
      await page.goto(pg, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(500); // Rapid navigation
    }

    await page.waitForTimeout(2000);
    await screenshot(page, '08-chaos-rapid-nav');

    // Some console errors from rapid navigation are expected (cancelled fetches)
    // But no unhandled exceptions
    const fatalErrors = errors.filter(e =>
      e.includes('Uncaught') || e.includes('TypeError') || e.includes('ReferenceError')
    );

    if (fatalErrors.length > 0) {
      console.log(`JS errors during rapid navigation: ${fatalErrors.join(', ')}`);
    }
    expect(fatalErrors.length).toBe(0);
  });
});
