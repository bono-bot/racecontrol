const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
  });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const outDir = 'tests/screenshots';

  // 1. Landing page
  await page.goto(base + '/');
  await page.waitForTimeout(2000);
  await page.screenshot({ path: outDir + '/01-kiosk-landing.png', fullPage: false });
  console.log('1. Landing page captured');

  // 2. Booking page (shows game selection)
  await page.goto(base + '/book');
  await page.waitForTimeout(2000);
  await page.screenshot({ path: outDir + '/02-kiosk-book.png', fullPage: false });
  console.log('2. Booking page captured');

  // 3. Staff booking with pod selection (shows game grid)
  await page.goto(base + '/book?pod=pod_8&staff=true');
  await page.waitForTimeout(3000);
  await page.screenshot({ path: outDir + '/03-staff-book-pod8.png', fullPage: false });
  console.log('3. Staff booking page captured');

  // 4. Try to click F1 25 if visible
  try {
    const f1Button = page.locator('text=F1 25').first();
    if (await f1Button.isVisible({ timeout: 3000 })) {
      await f1Button.click();
      await page.waitForTimeout(2000);
      await page.screenshot({ path: outDir + '/04-f1-25-selected.png', fullPage: false });
      console.log('4. F1 25 selected captured');
    } else {
      console.log('4. F1 25 button not visible on current step');
      await page.screenshot({ path: outDir + '/04-current-step.png', fullPage: false });
    }
  } catch (e) {
    console.log('4. Could not click F1 25:', e.message);
    await page.screenshot({ path: outDir + '/04-current-step.png', fullPage: false });
  }

  // 5. Control page (shows pod grid with game status)
  await page.goto(base + '/control');
  await page.waitForTimeout(2000);
  await page.screenshot({ path: outDir + '/05-control-page.png', fullPage: false });
  console.log('5. Control page captured');

  await browser.close();
  console.log('Done — screenshots in tests/screenshots/');
})();
