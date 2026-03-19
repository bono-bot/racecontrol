const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const out = 'tests/screenshots';

  // Staff walk-in
  await page.goto(base + '/book?pod=pod_8&staff=true');
  await page.waitForTimeout(2000);
  await page.locator('text=Walk-in (No Phone)').click();
  await page.waitForTimeout(2000);

  // Select 30 min
  await page.getByRole('button', { name: /30 Minutes/i }).click();
  await page.waitForTimeout(2000);

  // Game selection page - screenshot before clicking F1 25
  await page.screenshot({ path: out + '/debug-01-game-select.png' });

  // Check page title
  const title = await page.locator('h1, h2, .text-xl, .text-2xl').first().textContent();
  console.log('Current page title:', title);

  // Click F1 25
  const f1btn = page.locator('[data-testid="game-f1_25"]');
  const f1visible = await f1btn.isVisible({ timeout: 2000 }).catch(() => false);
  console.log('F1 25 button (data-testid) visible:', f1visible);

  if (!f1visible) {
    // Try text match
    const f1text = page.locator('button:has-text("F1 25")').first();
    console.log('F1 25 text button visible:', await f1text.isVisible({ timeout: 1000 }).catch(() => false));
    await f1text.click();
  } else {
    await f1btn.click();
  }

  await page.waitForTimeout(2000);
  await page.screenshot({ path: out + '/debug-02-after-f1-click.png' });

  // Check what's on the experience page
  const expTitle = await page.locator('h1, h2, .text-xl, .text-2xl, [class*="font-bold"]').first().textContent();
  console.log('After F1 click, page shows:', expTitle);

  // Count visible experiences
  const expItems = await page.locator('[data-testid^="experience-option"]').count();
  console.log('Experience items visible:', expItems);

  // Get all experience text
  const expTexts = await page.locator('[data-testid^="experience-option"] p').allTextContents();
  console.log('Experiences:', expTexts.join(' | '));

  await browser.close();
})();
