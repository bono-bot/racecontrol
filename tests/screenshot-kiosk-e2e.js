const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
  const page = await context.newPage();
  const base = 'http://192.168.31.23:8080/kiosk';
  const out = 'tests/screenshots';
  let pass = 0, fail = 0;

  function check(label, ok) {
    if (ok) { pass++; console.log(`  PASS: ${label}`); }
    else    { fail++; console.log(`  FAIL: ${label}`); }
  }

  // --- Test 1: Kiosk home page loads via :8080 proxy ---
  console.log('\n[1] Kiosk home page via proxy (:8080/kiosk)');
  const resp = await page.goto(base, { waitUntil: 'networkidle', timeout: 15000 });
  check('HTTP status 200', resp && resp.status() === 200);
  check('Page has content', (await page.content()).length > 500);
  await page.screenshot({ path: `${out}/e2e-01-kiosk-home.png`, fullPage: true });
  console.log(`  Screenshot: ${out}/e2e-01-kiosk-home.png`);

  // --- Test 2: Static assets load (CSS/_next) ---
  console.log('\n[2] Static assets via proxy');
  const styles = await page.evaluate(() => document.styleSheets.length);
  check('Stylesheets loaded', styles > 0);
  const nextScript = await page.evaluate(() =>
    [...document.querySelectorAll('script[src]')].some(s => s.src.includes('_next'))
  );
  check('Next.js scripts loaded', nextScript);

  // --- Test 3: Navigate to a pod booking page ---
  console.log('\n[3] Pod booking page');
  const bookResp = await page.goto(base + '/book?pod=pod_8&staff=true', { waitUntil: 'networkidle', timeout: 15000 });
  check('Book page loads (200)', bookResp && bookResp.status() === 200);
  await page.waitForTimeout(2000);
  await page.screenshot({ path: `${out}/e2e-02-kiosk-book.png`, fullPage: true });
  console.log(`  Screenshot: ${out}/e2e-02-kiosk-book.png`);

  // --- Test 4: Check control panel page ---
  console.log('\n[4] Control panel page');
  const ctrlResp = await page.goto(base + '/control', { waitUntil: 'networkidle', timeout: 15000 });
  check('Control page loads', ctrlResp && ctrlResp.status() === 200);
  await page.waitForTimeout(2000);
  await page.screenshot({ path: `${out}/e2e-03-kiosk-control.png`, fullPage: true });
  console.log(`  Screenshot: ${out}/e2e-03-kiosk-control.png`);

  // --- Test 5: Check staff page ---
  console.log('\n[5] Staff page');
  const staffResp = await page.goto(base + '/staff', { waitUntil: 'networkidle', timeout: 15000 });
  check('Staff page loads', staffResp && staffResp.status() === 200);
  await page.waitForTimeout(2000);
  await page.screenshot({ path: `${out}/e2e-04-kiosk-staff.png`, fullPage: true });
  console.log(`  Screenshot: ${out}/e2e-04-kiosk-staff.png`);

  // --- Test 6: API health through same port ---
  console.log('\n[6] API health through same :8080 port');
  const apiResp = await page.goto('http://192.168.31.23:8080/api/v1/health', { waitUntil: 'networkidle', timeout: 10000 });
  check('API health 200', apiResp && apiResp.status() === 200);
  const apiBody = await page.content();
  check('API returns JSON', apiBody.includes('"status"'));

  // --- Summary ---
  console.log(`\n========================================`);
  console.log(`E2E Results: ${pass} passed, ${fail} failed`);
  console.log(`========================================\n`);

  await browser.close();
  process.exit(fail > 0 ? 1 : 0);
})();
