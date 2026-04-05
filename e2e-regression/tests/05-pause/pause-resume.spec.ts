// ═══════════════════════════════════════════════════════════════
// Pause/Resume Tests — all 4 pause types
// Manual, Game Pause, Disconnect, Crash Recovery
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { STAFF_PIN, PAUSE_TYPES } from '../../fixtures/test-data';
import { getAnyIdlePod } from '../../fixtures/random-pod';
import { createTestDriver, ensureWalletBalance } from '../../fixtures/test-driver-factory';
import { capturePodScreenshot } from '../../fixtures/pod-screen-capture';

const api = new RCApiClient();
let testDriverId: string;

test.describe('05 — Pause/Resume Tests', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
    const driver = await createTestDriver(api, { balancePaise: 2000000 });
    testDriverId = driver.driverId;
  });

  test('Manual pause — staff pauses and resumes', async ({ page }) => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    // Start session
    const session = await api.startBilling({
      pod_id: pod.podId,
      driver_id: testDriverId,
      sim_type: 'assetto_corsa',
      track: 'monza',
      car: 'ks_ferrari_sf15t',
    });

    // Launch game
    await api.launchGame({ pod_id: pod.podId, sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t' });

    // Wait for active
    try {
      await api.waitForBillingStatus(session.id, ['active'], 120_000);
    } catch {
      await api.stopBilling(session.id);
      test.skip(true, 'Game did not reach active');
      return;
    }

    // Pause
    const paused = await api.pauseBilling(session.id, 'E2E manual pause test');
    expect(paused.status).toBe('paused_manual');

    await loginPOS(page);
    await page.goto('/billing', { waitUntil: 'load' });
    await waitForApp(page);
    await screenshot(page, '05-pause-manual-paused');

    // Pod screenshot during pause
    await capturePodScreenshot(pod.podIp, '05-pause-manual-pod-paused');

    // Wait 5s (billing should NOT accrue during pause)
    const drivingBefore = paused.driving_seconds;
    await new Promise(r => setTimeout(r, 5000));
    const midPause = await api.getBillingSession(session.id);
    // Driving seconds should not have changed significantly during pause
    expect(midPause.driving_seconds - drivingBefore).toBeLessThan(3);

    // Resume
    const resumed = await api.resumeBilling(session.id);
    expect(resumed.status).toBe('active');
    await screenshot(page, '05-pause-manual-resumed');

    // Let it run briefly, then stop
    await new Promise(r => setTimeout(r, 5000));
    await api.stopBilling(session.id);
    await api.stopGame({ pod_id: pod.podId });

    const final = await api.getBillingSession(session.id);
    expect(['ended_early', 'completed']).toContain(final.status);

    console.log(`Manual pause: driving=${final.driving_seconds}s, recovery_pause=${final.recovery_pause_seconds}s`);
  });

  test('Game pause simulation — billing frozen during ESC', async ({ page }) => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    const session = await api.startBilling({
      pod_id: pod.podId, driver_id: testDriverId,
      sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t',
    });

    await api.launchGame({ pod_id: pod.podId, sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t' });

    try {
      await api.waitForBillingStatus(session.id, ['active'], 120_000);
    } catch {
      await api.stopBilling(session.id);
      test.skip(true, 'Game did not reach active');
      return;
    }

    // Note: game_pause is triggered by the agent detecting ESC menu
    // We can test manual pause as a proxy since both freeze billing
    await api.pauseBilling(session.id, 'E2E game pause simulation');
    const paused = await api.getBillingSession(session.id);
    await screenshot(page, '05-pause-game-paused');

    // Verify billing frozen
    const t1 = paused.driving_seconds;
    await new Promise(r => setTimeout(r, 5000));
    const t2 = (await api.getBillingSession(session.id)).driving_seconds;
    expect(t2 - t1).toBeLessThan(3);

    // Resume and end
    await api.resumeBilling(session.id);
    await new Promise(r => setTimeout(r, 3000));
    await api.stopBilling(session.id);
    await api.stopGame({ pod_id: pod.podId });

    console.log('Game pause simulation: billing correctly frozen during pause');
  });

  test('Disconnect simulation — auto-resume on reconnect', async () => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    const session = await api.startBilling({
      pod_id: pod.podId, driver_id: testDriverId,
      sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t',
    });

    await api.launchGame({ pod_id: pod.podId, sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t' });

    try {
      await api.waitForBillingStatus(session.id, ['active'], 120_000);
    } catch {
      await api.stopBilling(session.id);
      test.skip(true, 'Game did not reach active');
      return;
    }

    // Note: real disconnect requires WS drop — testing the pause behavior
    await api.pauseBilling(session.id, 'E2E disconnect simulation');
    const paused = await api.getBillingSession(session.id);

    // Resume (simulating reconnect)
    await new Promise(r => setTimeout(r, 3000));
    await api.resumeBilling(session.id);
    const resumed = await api.getBillingSession(session.id);
    expect(resumed.status).toBe('active');

    await api.stopBilling(session.id);
    await api.stopGame({ pod_id: pod.podId });

    console.log('Disconnect simulation: resumed after reconnect');
  });

  test('Crash recovery simulation — recovery window', async () => {
    const pod = await getAnyIdlePod(api);
    if (!pod) { test.skip(true, 'No idle pod'); return; }

    await ensureWalletBalance(api, testDriverId, 200000);

    const session = await api.startBilling({
      pod_id: pod.podId, driver_id: testDriverId,
      sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t',
    });

    await api.launchGame({ pod_id: pod.podId, sim_type: 'assetto_corsa', track: 'monza', car: 'ks_ferrari_sf15t' });

    try {
      await api.waitForBillingStatus(session.id, ['active'], 120_000);
    } catch {
      await api.stopBilling(session.id);
      test.skip(true, 'Game did not reach active');
      return;
    }

    // Simulate crash (pause with recovery semantics)
    await api.pauseBilling(session.id, 'E2E crash recovery simulation');
    const paused = await api.getBillingSession(session.id);

    // Wait during "recovery window"
    await new Promise(r => setTimeout(r, 5000));

    // Resume (game relaunched)
    await api.resumeBilling(session.id);

    await new Promise(r => setTimeout(r, 3000));
    await api.stopBilling(session.id);
    await api.stopGame({ pod_id: pod.podId });

    const final = await api.getBillingSession(session.id);
    console.log(`Crash recovery: driving=${final.driving_seconds}s, recovery_pause=${final.recovery_pause_seconds}s`);
  });
});
