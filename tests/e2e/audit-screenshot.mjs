// Audit proof: screenshot kiosk pages showing active game on Pod 8
import { chromium } from 'playwright';

const KIOSK_BASE = 'http://192.168.31.23:3300/kiosk';
const API_BASE = 'http://192.168.31.23:8080/api/v1';
const SCREENSHOT_DIR = 'C:/Users/bono/racingpoint/racecontrol/tests/e2e/results';
const JWT = process.env.JWT;

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();

  // 1. Kiosk main page
  console.log('1. Capturing kiosk main page...');
  await page.goto(KIOSK_BASE, { waitUntil: 'networkidle', timeout: 15000 });
  await page.screenshot({ path: `${SCREENSHOT_DIR}/audit-kiosk-main.png`, fullPage: true });
  console.log('   Saved: audit-kiosk-main.png');

  // 2. Kiosk control panel (shows pod status)
  console.log('2. Capturing kiosk control panel...');
  await page.goto(`${KIOSK_BASE}/control`, { waitUntil: 'networkidle', timeout: 15000 });
  await page.screenshot({ path: `${SCREENSHOT_DIR}/audit-kiosk-control.png`, fullPage: true });
  console.log('   Saved: audit-kiosk-control.png');

  // 3. Fleet health API (shows game running)
  console.log('3. Capturing fleet health...');
  await page.goto(`${API_BASE}/fleet/health`, { waitUntil: 'networkidle', timeout: 10000 });
  await page.screenshot({ path: `${SCREENSHOT_DIR}/audit-fleet-health.png`, fullPage: true });
  console.log('   Saved: audit-fleet-health.png');

  // 4. Games active API (shows AC running on pod 8)
  console.log('4. Capturing games/active...');
  if (JWT) {
    await page.setExtraHTTPHeaders({ 'Authorization': `Bearer ${JWT}` });
  }
  await page.goto(`${API_BASE}/games/active`, { waitUntil: 'networkidle', timeout: 10000 });
  await page.screenshot({ path: `${SCREENSHOT_DIR}/audit-games-active.png`, fullPage: true });
  console.log('   Saved: audit-games-active.png');

  // 5. Games catalog API
  console.log('5. Capturing games/catalog...');
  await page.goto(`${API_BASE}/games/catalog`, { waitUntil: 'networkidle', timeout: 10000 });
  await page.screenshot({ path: `${SCREENSHOT_DIR}/audit-games-catalog.png`, fullPage: true });
  console.log('   Saved: audit-games-catalog.png');

  await browser.close();
  console.log('\nAll screenshots captured in:', SCREENSHOT_DIR);
}

main().catch(e => { console.error(e); process.exit(1); });
