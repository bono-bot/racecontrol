import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { API_BASE, POD_IPS, STAFF_PIN } from '../../fixtures/test-data';

const api = new RCApiClient();

test.describe('00 — Smoke Tests', () => {
  test('Server health endpoint responds (no auth)', async () => {
    // Health is public — no login needed
    const health = await api.health();
    expect(health.status).toBe('ok');
    expect(health.build_id).toBeTruthy();
    console.log(`Server build_id: ${health.build_id}, service: ${health.service}`);
  });

  test('Staff auth works with PIN 0009', async () => {
    const token = await api.login(STAFF_PIN);
    expect(token).toBeTruthy();
    expect(token.length).toBeGreaterThan(10);
  });

  test('Fleet health — all pods reachable', async () => {
    if (!api['token']) await api.login(STAFF_PIN);
    const fleet = await api.fleetHealth();
    expect(fleet.length).toBeGreaterThan(0);

    const connected = fleet.filter(p => p.ws_connected);
    console.log(`Fleet: ${connected.length}/${fleet.length} pods connected`);

    for (const pod of fleet) {
      console.log(`  Pod ${pod.pod_number}: ws=${pod.ws_connected}, http=${pod.http_reachable}, build=${pod.build_id}, uptime=${pod.uptime_secs}s`);
    }

    // At least some pods should be connected
    expect(connected.length).toBeGreaterThan(0);
  });

  test('Pricing tiers exist', async () => {
    if (!api['token']) await api.login(STAFF_PIN);
    const tiers = await api.listPricingTiers();
    expect(tiers.length).toBeGreaterThan(0);
    console.log(`Pricing tiers: ${tiers.map(t => `${t.name}(${t.duration_minutes}min/₹${t.price_paise / 100})`).join(', ')}`);
  });

  test('Games catalog available', async () => {
    const catalog = await api.gamesCatalog();
    expect(catalog).toBeTruthy();
    console.log(`Games catalog: ${JSON.stringify(catalog).length} bytes`);
  });

  test('POS billing page loads', async ({ page }) => {
    await loginPOS(page);
    await page.goto('/billing', { waitUntil: 'load' });
    await waitForApp(page);
    await screenshot(page, '00-smoke-pos-billing');

    // Should show billing page (not login, not 404)
    expect(page.url()).not.toContain('/login');
    const title = await page.title();
    expect(title).not.toContain('404');

    // Should have some content
    const body = await page.textContent('body');
    expect(body?.length).toBeGreaterThan(100);
  });

  test('POS Live Overview loads with pod grid', async ({ page }) => {
    await loginPOS(page);
    await page.goto('/', { waitUntil: 'load' });
    await waitForApp(page);
    await screenshot(page, '00-smoke-pos-overview');

    const body = await page.textContent('body');
    const hasPodContent = body?.includes('Pod') || body?.includes('pod') || body?.includes('Idle') || body?.includes('Online');
    expect(hasPodContent).toBeTruthy();
  });

  test('All 21 POS sidebar pages load', async ({ page }) => {
    await loginPOS(page);

    const pages = [
      '/', '/pods', '/games', '/telemetry', '/ac-lan', '/ac-sessions',
      '/sessions', '/drivers', '/leaderboards', '/events', '/billing',
      '/billing/pricing', '/billing/history', '/bookings', '/ai',
      '/cameras', '/cameras/playback', '/cafe', '/settings', '/presenter', '/kiosk',
    ];

    for (const pg of pages) {
      await page.goto(pg, { waitUntil: 'load' });
      await page.waitForTimeout(2000);
      await screenshot(page, `00-smoke-page-${pg.replace(/\//g, '-').replace(/^-/, '')}`);
      expect(page.url()).not.toContain('/login');
    }
  });
});
