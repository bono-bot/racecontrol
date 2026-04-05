// ═══════════════════════════════════════════════════════════════
// Driver & Wallet Tests — registration, topup, linked, minor, trial
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot } from '../../fixtures/screenshot-helper';
import { STAFF_PIN, PAYMENTS } from '../../fixtures/test-data';
import { createTestDriver, createLinkedDriverPair } from '../../fixtures/test-driver-factory';

const api = new RCApiClient();

test.describe('03 — Driver & Wallet Tests', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
  });

  test('Register new driver via API', async ({ page }) => {
    const driver = await createTestDriver(api, { balancePaise: 0 });
    expect(driver.driverId).toBeTruthy();

    // Verify in POS UI
    await loginPOS(page);
    await page.goto('/drivers', { waitUntil: 'load' });
    await waitForApp(page);
    await screenshot(page, '03-driver-registered');

    console.log(`Registered: ${driver.name} (${driver.driverId})`);
  });

  for (const method of PAYMENTS) {
    test(`Wallet topup via ${method}`, async ({ page }) => {
      const driver = await createTestDriver(api, { balancePaise: 0 });

      // Topup via API
      const result = await api.topupWallet(driver.driverId, {
        amount_paise: 100000, // ₹1000
        method,
        notes: `E2E test topup via ${method}`,
      });
      expect(result.balance_paise).toBe(100000);

      // Verify wallet balance
      const wallet = await api.getWallet(driver.driverId);
      expect(wallet.balance_paise).toBe(100000);

      // Verify on POS
      await loginPOS(page);
      await page.goto('/billing', { waitUntil: 'load' });
      await waitForApp(page);
      await screenshot(page, `03-topup-${method}`);

      console.log(`Topup ${method}: balance = ₹${wallet.balance_paise / 100}`);
    });
  }

  test('Multiple topups accumulate', async () => {
    const driver = await createTestDriver(api, { balancePaise: 0 });

    await api.topupWallet(driver.driverId, { amount_paise: 50000, method: 'cash' });
    await api.topupWallet(driver.driverId, { amount_paise: 30000, method: 'upi' });
    await api.topupWallet(driver.driverId, { amount_paise: 20000, method: 'card' });

    const wallet = await api.getWallet(driver.driverId);
    expect(wallet.balance_paise).toBe(100000);
    expect(wallet.total_credited_paise).toBe(100000);
  });

  test('Wallet transaction history tracks all topups', async () => {
    const driver = await createTestDriver(api, { balancePaise: 0 });

    await api.topupWallet(driver.driverId, { amount_paise: 50000, method: 'cash', notes: 'First topup' });
    await api.topupWallet(driver.driverId, { amount_paise: 25000, method: 'upi', notes: 'Second topup' });

    const txns = await api.walletTransactions(driver.driverId);
    expect(txns.length).toBeGreaterThanOrEqual(2);
  });

  test('Linked driver pair (parent-child)', async () => {
    const pair = await createLinkedDriverPair(api);
    expect(pair.parent.driverId).toBeTruthy();
    expect(pair.child.driverId).toBeTruthy();

    // Verify parent has funded wallet
    const parentWallet = await api.getWallet(pair.parent.driverId);
    expect(parentWallet.balance_paise).toBe(500000); // ₹5000

    console.log(`Linked pair: parent=${pair.parent.name}, child=${pair.child.name}`);
  });

  test('Trial session (5 min free)', async () => {
    const driver = await createTestDriver(api, { balancePaise: 0 });

    // Driver should be eligible for trial (has_used_trial = false)
    const driverInfo = await api.getDriver(driver.driverId);
    expect(driverInfo.has_used_trial).toBeFalsy();
  });
});
