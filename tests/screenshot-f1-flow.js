const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const out = 'tests/screenshots';

  // Step 1: Staff walk-in
  await page.goto(base + '/book?pod=pod_8&staff=true');
  await page.waitForTimeout(2000);
  await page.locator('text=Walk-in (No Phone)').click();
  await page.waitForTimeout(2000);
  await page.screenshot({ path: out + '/f1-01-select-plan.png' });
  console.log('1. Select Plan');

  // Step 2: Click "30 Minutes" tier
  await page.getByRole('button', { name: /30 Minutes/i }).click();
  await page.waitForTimeout(2000);
  await page.screenshot({ path: out + '/f1-02-select-game.png' });
  console.log('2. Select Game');

  // Step 3: Click F1 25 game
  try {
    await page.locator('text=F1 25').first().click();
    await page.waitForTimeout(2000);
    await page.screenshot({ path: out + '/f1-03-select-experience.png' });
    console.log('3. Select Experience (F1 25)');
  } catch (e) {
    console.log('3. Could not find F1 25 button:', e.message);
    await page.screenshot({ path: out + '/f1-03-current.png' });
  }

  // Step 4: Click first available experience for F1 25
  try {
    const exp = page.locator('text=Practice').first();
    if (await exp.isVisible({ timeout: 3000 })) {
      await exp.click();
      await page.waitForTimeout(2000);
      await page.screenshot({ path: out + '/f1-04-review.png' });
      console.log('4. Review page');
    } else {
      await page.screenshot({ path: out + '/f1-04-current.png' });
      console.log('4. No Practice button found');
    }
  } catch (e) {
    await page.screenshot({ path: out + '/f1-04-current.png' });
    console.log('4. Error:', e.message);
  }

  // Extra: Control page showing pods
  await page.goto(base + '/control');
  await page.waitForTimeout(3000);
  await page.screenshot({ path: out + '/f1-05-control.png' });
  console.log('5. Control page');

  await browser.close();
  console.log('Done');
})();
