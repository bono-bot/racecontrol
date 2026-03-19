const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const out = 'tests/screenshots';

  // Check game selection for Pod 1 (missing F1 25, LMU, Forza) and Pod 8 (missing LMU, Forza)
  for (const pod of ['pod_1', 'pod_8']) {
    await page.goto(`${base}/book?pod=${pod}&staff=true`);
    await page.waitForTimeout(2000);
    await page.locator('text=Walk-in (No Phone)').click();
    await page.waitForTimeout(2000);
    await page.getByRole('button', { name: /30 Minutes/i }).click();
    await page.waitForTimeout(2000);
    await page.screenshot({ path: `${out}/games-${pod}.png` });
    console.log(`${pod}: game selection captured`);

    // Count visible game buttons
    const gameButtons = await page.locator('button').allTextContents();
    const games = gameButtons.filter(t =>
      ['Assetto Corsa', 'AC EVO', 'AC Rally', 'F1 25', 'iRacing', 'Le Mans', 'Forza'].some(g => t.includes(g))
    );
    console.log(`  Games shown: ${games.join(', ')}`);
  }

  await browser.close();
  console.log('Done');
})();
