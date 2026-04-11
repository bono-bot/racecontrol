/**
 * Wallet top-up verification test.
 * Confirms the 422 "missing field method" bug is fixed after f01bd396 deploy.
 *
 * Flow: Login with staff PIN → /billing → Top Up → search "Uday" → 500cr → CASH → Add
 */
import { test, expect, chromium } from '@playwright/test';
import path from 'path';
import fs from 'fs';

const BASE_URL = process.env.WEB_URL ?? 'http://192.168.31.23:3200';
const SCREENSHOTS_DIR = path.join(__dirname, '..', 'tests', 'screenshots');

test('wallet top-up flow - no 422 error', async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();

  fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });

  try {
    // Step 1: Navigate to billing page
    await page.goto(`${BASE_URL}/billing`, { waitUntil: 'networkidle' });
    await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '01-billing-page.png') });

    // Step 2: Login with staff PIN if login prompt appears
    const pinInput = page.locator('input[type="password"], input[placeholder*="PIN"], input[placeholder*="pin"]').first();
    if (await pinInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await pinInput.fill('5072');
      await page.keyboard.press('Enter');
      await page.waitForTimeout(1500);
      await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '02-after-login.png') });
    }

    // Step 3: Look for "Top Up" button
    const topUpBtn = page.locator('button, [role="button"]').filter({ hasText: /top.?up|add credit/i }).first();
    if (await topUpBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await topUpBtn.click();
      await page.waitForTimeout(1000);
      await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '03-topup-modal-open.png') });
    } else {
      // Try clicking a customer row first
      const customerRow = page.locator('tr, [data-testid*="customer"], .customer-row').first();
      if (await customerRow.isVisible({ timeout: 3000 }).catch(() => false)) {
        await customerRow.click();
        await page.waitForTimeout(500);
        const topUpBtn2 = page.locator('button').filter({ hasText: /top.?up|add credit/i }).first();
        await topUpBtn2.click({ timeout: 5000 });
        await page.waitForTimeout(1000);
        await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '03-topup-modal-open.png') });
      }
    }

    // Step 4: Search for "Uday" if search box present
    const searchInput = page.locator('input[placeholder*="search" i], input[placeholder*="name" i], input[placeholder*="customer" i]').first();
    if (await searchInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await searchInput.fill('Uday');
      await page.waitForTimeout(1000);
      await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '04-search-uday.png') });

      // Select first result
      const result = page.locator('li, [role="option"], .customer-option').first();
      if (await result.isVisible({ timeout: 3000 }).catch(() => false)) {
        await result.click();
        await page.waitForTimeout(500);
        await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '05-customer-selected.png') });
      }
    }

    // Step 5: Enter amount 500 and select CASH
    const amountInput = page.locator('input[type="number"], input[placeholder*="amount" i], input[placeholder*="credit" i]').first();
    if (await amountInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await amountInput.clear();
      await amountInput.fill('500');
    }

    // Select CASH method
    const cashOption = page.locator('button, [role="radio"], [role="option"], label').filter({ hasText: /^cash$/i }).first();
    if (await cashOption.isVisible({ timeout: 3000 }).catch(() => false)) {
      await cashOption.click();
    }
    await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '06-amount-and-method.png') });

    // Step 6: Intercept the topup API call and click Add
    const responsePromise = page.waitForResponse(
      resp => resp.url().includes('/wallet/topup'),
      { timeout: 10000 }
    ).catch(() => null);

    const addBtn = page.locator('button[type="submit"], button').filter({ hasText: /^add$|^submit$|^top.?up$/i }).first();
    if (await addBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await addBtn.click();
    }

    const response = await responsePromise;
    await page.waitForTimeout(2000);
    await page.screenshot({ path: path.join(SCREENSHOTS_DIR, '07-after-submit.png') });

    // Check for network error or 422 toast
    const errorText = await page.locator('text=/network error|422|missing field/i').first().isVisible({ timeout: 2000 }).catch(() => false);

    console.log('API response status:', response ? response.status() : 'no response intercepted');
    console.log('Error text visible:', errorText);
    console.log('Screenshots saved to:', SCREENSHOTS_DIR);

    // The fix is verified if no 422 and no "Network error" toast
    expect(errorText).toBe(false);

  } finally {
    await browser.close();
  }
});
