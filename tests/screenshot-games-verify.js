const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const out = 'tests/screenshots';

  // Pod 1 (no F1 25) vs Pod 8 (has F1 25) — should show different game lists
  for (const [pod, expected] of [
    ['pod_1', 'NO F1 25, NO LMU — AC, EVO, Rally, iRacing only'],
    ['pod_6', 'NO EVO, NO Rally — AC, F1 25, iRacing only'],
    ['pod_7', 'MOST games — AC, EVO, Rally, F1 25, iRacing'],
  ]) {
    await page.goto(`${base}/book?pod=${pod}&staff=true`);
    await page.waitForTimeout(2000);
    await page.locator('text=Walk-in (No Phone)').click();
    await page.waitForTimeout(2000);
    await page.getByRole('button', { name: /30 Minutes/i }).click();
    await page.waitForTimeout(2000);
    await page.screenshot({ path: `${out}/verify-${pod}-games.png` });

    // Extract visible game names
    const buttons = await page.locator('button').allTextContents();
    const games = buttons.filter(t =>
      ['Assetto Corsa', 'AC EVO', 'AC Rally', 'F1 25', 'iRacing', 'Le Mans', 'Forza'].some(g => t.includes(g))
    );
    console.log(`${pod} (${expected}):`);
    console.log(`  Shown: ${games.join(', ')}`);
    const hasF1 = games.some(g => g.includes('F1 25'));
    const hasLMU = games.some(g => g.includes('Le Mans'));
    const hasEVO = games.some(g => g.includes('EVO'));
    if (pod === 'pod_1' && !hasF1 && !hasLMU) console.log('  VERIFIED: F1 25 and LMU correctly hidden');
    if (pod === 'pod_1' && (hasF1 || hasLMU)) console.log('  BUG: Ghost game visible!');
    if (pod === 'pod_6' && !hasEVO) console.log('  VERIFIED: AC EVO correctly hidden');
    if (pod === 'pod_6' && hasEVO) console.log('  BUG: Ghost game visible!');
    console.log('');
  }

  await browser.close();
  console.log('Done');
})();
