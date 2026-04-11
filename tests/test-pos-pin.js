// Test PIN entry on the billing page — simulates what the POS user does
const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });

  // Navigate to billing page (same URL as POS)
  await page.goto('http://192.168.31.23:3200/billing', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(3000);

  // Screenshot before PIN entry
  await page.screenshot({ path: 'C:/Users/bono/pos-test-before.png' });
  console.log('Before PIN: screenshot saved');

  // Check if we're on the login page
  const pinPad = await page.$('[aria-label="PIN entry"]');
  if (!pinPad) {
    console.log('ERROR: No PIN pad found on the page');
    // Check if already logged in or different page
    const bodyText = await page.textContent('body');
    console.log('Page text (first 200 chars):', bodyText.substring(0, 200));
    await browser.close();
    process.exit(1);
  }

  console.log('PIN pad found, entering 7080 (TEST_ONLY_E2E)...');

  // Intercept the API call to see exactly what's sent
  page.on('request', req => {
    if (req.url().includes('validate-pin')) {
      console.log(`API REQUEST: ${req.method()} ${req.url()}`);
      console.log(`BODY: ${req.postData()}`);
    }
  });
  page.on('response', res => {
    if (res.url().includes('validate-pin')) {
      console.log(`API RESPONSE: ${res.status()}`);
      res.text().then(t => console.log(`RESPONSE BODY: ${t}`));
    }
  });

  // Enter PIN by clicking the buttons: 7, 0, 8, 0
  const buttons = await page.$$('button');
  const buttonMap = {};
  for (const btn of buttons) {
    const text = await btn.textContent();
    buttonMap[text.trim()] = btn;
  }

  console.log('Available buttons:', Object.keys(buttonMap).join(', '));

  // Click 7, 0, 8, 0 (TEST_ONLY_E2E staff)
  for (const digit of ['7', '0', '8', '0']) {
    if (buttonMap[digit]) {
      await buttonMap[digit].click();
      await page.waitForTimeout(200);
      console.log(`Clicked: ${digit}`);
    } else {
      console.log(`ERROR: Button '${digit}' not found!`);
    }
  }

  // Wait for auto-submit and response
  await page.waitForTimeout(3000);

  // Screenshot after PIN entry
  await page.screenshot({ path: 'C:/Users/bono/pos-test-after.png' });
  console.log('After PIN: screenshot saved');

  // Check current URL (did it redirect?)
  console.log('Current URL:', page.url());

  // Check for error message
  const errorEl = await page.$('[role="alert"]');
  if (errorEl) {
    const errorText = await errorEl.textContent();
    console.log('ERROR SHOWN:', errorText);
  } else {
    console.log('No error shown — login may have succeeded');
  }

  await browser.close();
})();
