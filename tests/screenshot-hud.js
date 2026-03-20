const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const out = 'tests/screenshots';

  // 1. Spectator page
  await page.goto(base + '/spectator');
  await page.waitForTimeout(4000);
  await page.screenshot({ path: out + '/hud-01-spectator.png' });
  console.log('1. Spectator page captured');

  // Check live elements
  const podCards = await page.locator('[class*="pod"], [class*="rig"], tr').count();
  console.log('   Pod/rig elements:', podCards);
  const wsIndicator = await page.locator('text=Live, text=Connected, [class*="green"]').count();
  console.log('   Live indicators:', wsIndicator);

  // 2. Presenter page
  await page.goto('http://192.168.31.23:8080/presenter');
  await page.waitForTimeout(3000);
  await page.screenshot({ path: out + '/hud-02-presenter.png' });
  console.log('2. Presenter page captured');

  // 3. Leaderboards page
  await page.goto('http://192.168.31.23:8080/leaderboards');
  await page.waitForTimeout(3000);
  await page.screenshot({ path: out + '/hud-03-leaderboards.png' });
  console.log('3. Leaderboards page captured');

  // 4. Control page (shows live pod states)
  await page.goto(base + '/control');
  await page.waitForTimeout(3000);
  await page.screenshot({ path: out + '/hud-04-control.png' });
  console.log('4. Control page captured');

  await browser.close();
  console.log('Done');
})();
