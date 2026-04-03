#!/usr/bin/env node
/**
 * screenshot-url.js — Headless browser screenshot for visual verification
 *
 * Usage:
 *   node scripts/screenshot-url.js <url> [output_path] [--width=1024] [--height=768] [--wait=3000] [--full]
 *
 * Examples:
 *   node scripts/screenshot-url.js http://192.168.31.23:3200/billing
 *   node scripts/screenshot-url.js http://192.168.31.23:3300/kiosk/staff /tmp/staff.png --width=1920 --height=1080
 *   node scripts/screenshot-url.js http://192.168.31.23:3300/kiosk --full
 *
 * Outputs screenshot path on success. View with Claude Code's Read tool.
 */

const { chromium } = require('playwright');
const path = require('path');

async function main() {
  const args = process.argv.slice(2);
  if (args.length === 0 || args[0] === '--help') {
    console.log('Usage: node screenshot-url.js <url> [output_path] [--width=N] [--height=N] [--wait=ms] [--full]');
    process.exit(0);
  }

  // Parse args
  const positional = args.filter(a => !a.startsWith('--'));
  const flags = Object.fromEntries(
    args.filter(a => a.startsWith('--')).map(a => {
      const [k, v] = a.slice(2).split('=');
      return [k, v === undefined ? true : v];
    })
  );

  const url = positional[0];
  const width = parseInt(flags.width || '1024');
  const height = parseInt(flags.height || '768');
  const waitMs = parseInt(flags.wait || '3000');
  const fullPage = !!flags.full;

  // Generate output path from URL if not specified
  const defaultName = url.replace(/https?:\/\//, '').replace(/[^a-zA-Z0-9]/g, '_').slice(0, 60);
  const outPath = positional[1] || path.join(process.env.TEMP || '/tmp', `screenshot_${defaultName}.png`);

  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
      viewport: { width, height },
      ignoreHTTPSErrors: true,
    });
    const page = await context.newPage();

    // Navigate and wait for network to settle
    await page.goto(url, { waitUntil: 'networkidle', timeout: 15000 }).catch(() => {
      // Fall back to domcontentloaded if networkidle times out (e.g., long-polling WS)
      return page.goto(url, { waitUntil: 'domcontentloaded', timeout: 10000 });
    });

    // Additional wait for React hydration / client-side rendering
    await page.waitForTimeout(waitMs);

    // Take screenshot
    await page.screenshot({ path: outPath, fullPage });

    console.log(outPath);
    process.exit(0);
  } catch (err) {
    console.error(`SCREENSHOT_FAIL: ${err.message}`);
    process.exit(1);
  } finally {
    if (browser) await browser.close();
  }
}

main();
