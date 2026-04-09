// Captures N frames from http://192.168.31.23:3300/kiosk/blanking
// for embedding into rc-agent as a native GDI animation.
// Output: crates/rc-agent/assets/blanking-frames/frame-NN.png

const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const URL = 'http://192.168.31.23:3300/kiosk/blanking';
const OUTPUT_DIR = path.join(__dirname, '..', '..', 'crates', 'rc-agent', 'assets', 'blanking-frames');
// 960x540 downscale: 30 frames * 960 * 540 * 4 = ~62 MB decoded HBITMAP memory.
// StretchBlt upscales to physical monitor at runtime (2560x1440 middle monitor).
const WIDTH = 960;
const HEIGHT = 540;
const FRAMES = 24;
const INTERVAL_MS = 125; // 125ms * 24 = 3-second loop
const WARMUP_MS = 1000;  // Let ember particles stabilize

(async () => {
  if (!fs.existsSync(OUTPUT_DIR)) fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  console.log(`Capturing ${FRAMES} frames at ${WIDTH}x${HEIGHT} from ${URL}`);
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 });
  const page = await ctx.newPage();

  await page.goto(URL, { waitUntil: 'networkidle', timeout: 15000 });
  console.log(`  Page loaded, warming up ${WARMUP_MS}ms`);
  await page.waitForTimeout(WARMUP_MS);

  const t0 = Date.now();
  for (let i = 0; i < FRAMES; i++) {
    const file = path.join(OUTPUT_DIR, `frame-${String(i).padStart(2, '0')}.png`);
    await page.screenshot({ path: file, type: 'png', omitBackground: false });
    const sz = fs.statSync(file).size;
    console.log(`  frame-${String(i).padStart(2, '0')}.png  ${sz} bytes`);
    if (i < FRAMES - 1) await page.waitForTimeout(INTERVAL_MS);
  }
  const elapsed = Date.now() - t0;
  console.log(`Done in ${elapsed}ms`);

  // Total size
  const files = fs.readdirSync(OUTPUT_DIR).filter(f => f.endsWith('.png'));
  const total = files.reduce((s, f) => s + fs.statSync(path.join(OUTPUT_DIR, f)).size, 0);
  console.log(`Total: ${files.length} frames, ${(total / 1024 / 1024).toFixed(2)} MB`);

  await browser.close();
})().catch(e => { console.error('ERROR:', e); process.exit(1); });
