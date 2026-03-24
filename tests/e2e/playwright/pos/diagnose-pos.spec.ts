import { test, expect } from '@playwright/test';

test('diagnose dashboard (:3200)', async ({ page }) => {
  const errors: string[] = [];
  const failedRequests: string[] = [];
  const wsEvents: string[] = [];

  // Capture JS errors
  page.on('pageerror', (err) => errors.push(err.message));

  // Capture failed network requests
  page.on('requestfailed', (req) => {
    failedRequests.push(`${req.method()} ${req.url()} → ${req.failure()?.errorText}`);
  });

  // Capture console messages
  page.on('console', (msg) => {
    if (msg.type() === 'error' || msg.type() === 'warn') {
      wsEvents.push(`[${msg.type()}] ${msg.text()}`);
    }
  });

  // Navigate
  await page.goto('http://192.168.31.23:3200', { waitUntil: 'networkidle', timeout: 30000 });

  // Wait for potential JS hydration
  await page.waitForTimeout(5000);

  // Screenshot full page
  await page.screenshot({ path: 'test-results/dashboard-full.png', fullPage: true });

  // Check for visible text
  const bodyText = await page.textContent('body') ?? '';
  console.log('--- DASHBOARD BODY TEXT (first 500 chars) ---');
  console.log(bodyText.slice(0, 500));

  console.log('\n--- JS ERRORS ---');
  errors.forEach((e) => console.log(`  ERROR: ${e}`));
  if (!errors.length) console.log('  (none)');

  console.log('\n--- FAILED REQUESTS ---');
  failedRequests.forEach((r) => console.log(`  FAIL: ${r}`));
  if (!failedRequests.length) console.log('  (none)');

  console.log('\n--- CONSOLE WARNINGS/ERRORS ---');
  wsEvents.forEach((e) => console.log(`  ${e}`));
  if (!wsEvents.length) console.log('  (none)');

  // Check for blank/error state
  const hasContent = bodyText.length > 100;
  console.log(`\n--- DIAGNOSIS: body length=${bodyText.length}, hasContent=${hasContent} ---`);

  expect(hasContent).toBe(true);
});

test('diagnose kiosk (:3300)', async ({ page }) => {
  const errors: string[] = [];
  const failedRequests: string[] = [];
  const consoleMessages: string[] = [];

  page.on('pageerror', (err) => errors.push(err.message));
  page.on('requestfailed', (req) => {
    failedRequests.push(`${req.method()} ${req.url()} → ${req.failure()?.errorText}`);
  });
  page.on('console', (msg) => {
    if (msg.type() === 'error' || msg.type() === 'warn') {
      consoleMessages.push(`[${msg.type()}] ${msg.text()}`);
    }
  });

  await page.goto('http://192.168.31.23:3300', { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(5000);

  await page.screenshot({ path: 'test-results/kiosk-full.png', fullPage: true });

  const bodyText = await page.textContent('body') ?? '';
  console.log('--- KIOSK BODY TEXT (first 500 chars) ---');
  console.log(bodyText.slice(0, 500));

  // Check WS status indicator
  const wsStatus = await page.locator('[data-testid="ws-status"]').textContent().catch(() => 'NOT FOUND');
  console.log(`\n--- WS STATUS: ${wsStatus} ---`);

  // Check pod count
  const availableText = await page.locator('text=Available').first().textContent().catch(() => 'NOT FOUND');
  const racingText = await page.locator('text=Racing').first().textContent().catch(() => 'NOT FOUND');
  console.log(`--- AVAILABLE: ${availableText} ---`);
  console.log(`--- RACING: ${racingText} ---`);

  // Check pod grid
  const podGrid = await page.locator('[data-testid="pod-grid"]').innerHTML().catch(() => 'NOT FOUND');
  const podCount = (podGrid.match(/Pod \d/g) || []).length;
  console.log(`--- POD GRID: ${podCount} pods found ---`);

  console.log('\n--- JS ERRORS ---');
  errors.forEach((e) => console.log(`  ERROR: ${e}`));
  if (!errors.length) console.log('  (none)');

  console.log('\n--- FAILED REQUESTS ---');
  failedRequests.forEach((r) => console.log(`  FAIL: ${r}`));
  if (!failedRequests.length) console.log('  (none)');

  console.log('\n--- CONSOLE WARNINGS/ERRORS ---');
  consoleMessages.forEach((e) => console.log(`  ${e}`));
  if (!consoleMessages.length) console.log('  (none)');

  const hasContent = bodyText.length > 100;
  console.log(`\n--- DIAGNOSIS: body length=${bodyText.length}, hasContent=${hasContent} ---`);
});

test('diagnose dashboard billing page (:3200/billing)', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (err) => errors.push(err.message));
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(`[console] ${msg.text()}`);
  });

  await page.goto('http://192.168.31.23:3200/billing', { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(5000);
  await page.screenshot({ path: 'test-results/billing-full.png', fullPage: true });

  const bodyText = await page.textContent('body') ?? '';
  console.log('--- BILLING PAGE BODY (first 500) ---');
  console.log(bodyText.slice(0, 500));

  console.log('\n--- JS ERRORS ---');
  errors.forEach((e) => console.log(`  ${e}`));
  if (!errors.length) console.log('  (none)');
});
