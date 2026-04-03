#!/usr/bin/env node
/**
 * flow-verify.js — Playwright interactive flow verification for Racing Point
 *
 * Runs multi-step user journeys through the browser, clicking buttons,
 * filling forms, and asserting page state at each step. Screenshots every step.
 *
 * Usage:
 *   node scripts/flow-verify.js                    # Run all flows
 *   node scripts/flow-verify.js kiosk-to-staff     # Run one flow
 *   node scripts/flow-verify.js --list             # List available flows
 *   node scripts/flow-verify.js --headed           # Watch in real browser (debug)
 *   node scripts/flow-verify.js --slow             # 1s delay between steps (debug)
 *
 * Exit codes: 0 = all PASS, 1 = FAIL found
 */

const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const SERVER = process.env.RC_SERVER || '192.168.31.23';

// ─── Flow Definitions ───────────────────────────────────────────────────────
// Each flow is an array of steps. Each step has:
//   action: 'goto'|'click'|'fill'|'wait'|'assert-text'|'assert-no-text'|
//           'assert-url'|'assert-visible'|'assert-hidden'|'screenshot'|'key'
//   target: CSS selector or URL or text (depends on action)
//   value:  fill value, wait ms, or expected text
//   label:  human-readable step name

const FLOWS = {
  'kiosk-to-staff': {
    name: 'Kiosk → Staff Login navigation',
    viewport: { width: 1024, height: 768 },
    steps: [
      { action: 'goto', target: `http://${SERVER}:3300/kiosk`, label: 'Open customer kiosk' },
      { action: 'wait', value: 3000, label: 'Wait for hydration' },
      { action: 'screenshot', label: 'Customer page loaded' },
      { action: 'assert-text', value: 'Staff Login', label: 'Staff Login link visible' },
      { action: 'assert-text', value: 'Choose Your Rig', label: 'Customer header present' },
      { action: 'click', target: 'text=Staff Login', label: 'Click Staff Login' },
      { action: 'wait', value: 4000, label: 'Wait for staff page' },
      { action: 'screenshot', label: 'Staff page loaded' },
      { action: 'assert-text', value: 'Sign In', label: 'Sign In prompt visible' },
      { action: 'assert-no-text', value: 'Choose Your Rig', label: 'Not customer page' },
      { action: 'assert-url', value: '/staff', label: 'URL contains /staff' },
    ],
  },

  'staff-to-customer': {
    name: 'Staff Login → Customer Login navigation',
    viewport: { width: 1024, height: 768 },
    steps: [
      { action: 'goto', target: `http://${SERVER}:3300/kiosk/staff`, label: 'Open staff page' },
      { action: 'wait', value: 4000, label: 'Wait for hydration' },
      { action: 'screenshot', label: 'Staff login loaded' },
      { action: 'assert-text', value: 'Sign In', label: 'Sign In visible' },
      { action: 'click', target: 'text=Customer Login', label: 'Click Customer Login' },
      { action: 'wait', value: 3000, label: 'Wait for navigation' },
      { action: 'screenshot', label: 'Customer page loaded' },
      { action: 'assert-text', value: 'Staff Login', label: 'Back on customer page' },
    ],
  },

  'billing-pin': {
    name: 'Billing PIN pad interaction',
    viewport: { width: 1024, height: 768 },
    steps: [
      { action: 'goto', target: `http://${SERVER}:3200/billing`, label: 'Open billing page' },
      { action: 'wait', value: 3000, label: 'Wait for hydration' },
      { action: 'screenshot', label: 'PIN pad loaded' },
      { action: 'assert-text', value: 'PIN', label: 'PIN prompt visible' },
      { action: 'assert-visible', target: 'button', label: 'Buttons rendered' },
      // Click digits 1, 2, 3, 4
      { action: 'click', target: 'button:has-text("1")', label: 'Press 1' },
      { action: 'click', target: 'button:has-text("2")', label: 'Press 2' },
      { action: 'click', target: 'button:has-text("3")', label: 'Press 3' },
      { action: 'click', target: 'button:has-text("4")', label: 'Press 4' },
      { action: 'wait', value: 1000, label: 'Wait for PIN submit' },
      { action: 'screenshot', label: 'After PIN entry' },
      // CLR button should exist
      { action: 'assert-visible', target: 'button:has-text("CLR")', label: 'CLR button present' },
    ],
  },

  'kiosk-pod-click': {
    name: 'Kiosk pod selection',
    viewport: { width: 1920, height: 1080 },
    steps: [
      { action: 'goto', target: `http://${SERVER}:3300/kiosk`, label: 'Open kiosk' },
      { action: 'wait', value: 4000, label: 'Wait for pods to load' },
      { action: 'screenshot', label: 'Pods loaded' },
      { action: 'assert-visible', target: '[data-testid="pod-grid"]', label: 'Pod grid visible' },
      // Click first pod
      { action: 'click', target: '[data-testid="pod-grid"] > div:first-child', label: 'Click Pod 1' },
      { action: 'wait', value: 2000, label: 'Wait for pod detail' },
      { action: 'screenshot', label: 'After pod click' },
    ],
  },

  'portal-navigation': {
    name: 'Portal status page',
    viewport: { width: 1024, height: 768 },
    steps: [
      { action: 'goto', target: `http://${SERVER}:8080/`, label: 'Open portal root' },
      { action: 'wait', value: 3000, label: 'Wait for load' },
      { action: 'screenshot', label: 'Portal loaded' },
      { action: 'assert-no-text', value: 'Cannot GET', label: 'Not a 404' },
    ],
  },
};

// ─── Step Executor ──────────────────────────────────────────────────────────

async function executeStep(page, step, screenshotDir, stepIndex) {
  const result = { label: step.label, action: step.action, pass: true, detail: '' };

  try {
    switch (step.action) {
      case 'goto':
        await page.goto(step.target, { waitUntil: 'domcontentloaded', timeout: 12000 });
        result.detail = step.target;
        break;

      case 'click':
        await page.locator(step.target).first().click({ timeout: 5000 });
        result.detail = step.target;
        break;

      case 'fill':
        await page.locator(step.target).first().fill(step.value, { timeout: 5000 });
        result.detail = `${step.target} = "${step.value}"`;
        break;

      case 'key':
        await page.keyboard.press(step.value);
        result.detail = step.value;
        break;

      case 'wait':
        await page.waitForTimeout(step.value);
        result.detail = `${step.value}ms`;
        break;

      case 'screenshot': {
        const fname = `step_${String(stepIndex).padStart(2, '0')}_${step.label.replace(/[^a-zA-Z0-9]/g, '_').toLowerCase()}.png`;
        const fpath = path.join(screenshotDir, fname);
        await page.screenshot({ path: fpath });
        result.detail = fpath;
        result.screenshot = fpath;
        break;
      }

      case 'assert-text': {
        const body = await page.textContent('body').catch(() => '');
        const has = body.includes(step.value);
        result.pass = has;
        result.detail = has ? `"${step.value}" found` : `"${step.value}" NOT FOUND`;
        break;
      }

      case 'assert-no-text': {
        const body = await page.textContent('body').catch(() => '');
        const has = body.includes(step.value);
        result.pass = !has;
        result.detail = has ? `"${step.value}" FOUND (should be absent)` : `"${step.value}" absent`;
        break;
      }

      case 'assert-url': {
        const url = page.url();
        const has = url.includes(step.value);
        result.pass = has;
        result.detail = has ? `URL contains "${step.value}"` : `URL "${url}" missing "${step.value}"`;
        break;
      }

      case 'assert-visible': {
        const vis = await page.locator(step.target).first().isVisible({ timeout: 3000 }).catch(() => false);
        result.pass = vis;
        result.detail = vis ? `${step.target} visible` : `${step.target} NOT VISIBLE`;
        break;
      }

      case 'assert-hidden': {
        const vis = await page.locator(step.target).first().isVisible({ timeout: 3000 }).catch(() => false);
        result.pass = !vis;
        result.detail = vis ? `${step.target} VISIBLE (should be hidden)` : `${step.target} hidden`;
        break;
      }

      default:
        result.detail = `Unknown action: ${step.action}`;
        result.pass = false;
    }
  } catch (err) {
    result.pass = false;
    result.detail = `Error: ${err.message.split('\n')[0]}`;
  }

  return result;
}

// ─── Flow Runner ────────────────────────────────────────────────────────────

async function runFlow(browser, flowId, flowDef, screenshotDir, slow) {
  const flowResult = {
    id: flowId, name: flowDef.name, pass: true,
    steps: [], screenshots: [], duration: 0,
  };
  const start = Date.now();
  const flowDir = path.join(screenshotDir, flowId);
  fs.mkdirSync(flowDir, { recursive: true });

  const ctx = await browser.newContext({
    viewport: flowDef.viewport || { width: 1024, height: 768 },
    ignoreHTTPSErrors: true,
  });
  const page = await ctx.newPage();

  for (let i = 0; i < flowDef.steps.length; i++) {
    const step = flowDef.steps[i];
    if (slow) await page.waitForTimeout(1000);

    const result = await executeStep(page, step, flowDir, i);
    flowResult.steps.push(result);

    if (result.screenshot) flowResult.screenshots.push(result.screenshot);
    if (!result.pass) {
      flowResult.pass = false;
      // Take failure screenshot
      const failPath = path.join(flowDir, `FAIL_step_${String(i).padStart(2, '0')}.png`);
      await page.screenshot({ path: failPath }).catch(() => {});
      flowResult.screenshots.push(failPath);
      break; // Stop flow on first failure
    }
  }

  await ctx.close();
  flowResult.duration = Date.now() - start;
  return flowResult;
}

// ─── Main ───────────────────────────────────────────────────────────────────

async function main() {
  const rawArgs = process.argv.slice(2);
  const flags = rawArgs.filter(a => a.startsWith('--'));
  const targets = rawArgs.filter(a => !a.startsWith('--'));

  if (flags.includes('--list')) {
    console.log('\nAvailable flows:\n');
    for (const [id, def] of Object.entries(FLOWS)) {
      console.log(`  ${id.padEnd(25)} ${def.name} (${def.steps.length} steps)`);
    }
    console.log();
    process.exit(0);
  }

  const headed = flags.includes('--headed');
  const slow = flags.includes('--slow');

  // Select flows
  let flowIds = targets.length > 0 ? targets : Object.keys(FLOWS);
  flowIds = flowIds.filter(id => {
    if (!FLOWS[id]) { console.error(`Unknown flow: ${id}`); return false; }
    return true;
  });
  if (flowIds.length === 0) process.exit(1);

  const screenshotDir = path.join(process.env.TEMP || '/tmp', `flow-verify-${Date.now()}`);
  fs.mkdirSync(screenshotDir, { recursive: true });

  console.log(`\n  Flow Verify — ${flowIds.length} flow(s)${headed ? ' [headed]' : ''}${slow ? ' [slow]' : ''}\n`);

  const browser = await chromium.launch({ headless: !headed, slowMo: slow ? 500 : 0 });
  const results = [];

  for (const flowId of flowIds) {
    const flowDef = FLOWS[flowId];
    process.stdout.write(`  ${flowDef.name} ... `);
    const r = await runFlow(browser, flowId, flowDef, screenshotDir, slow);
    results.push(r);

    console.log(r.pass ? `PASS (${r.duration}ms)` : 'FAIL');

    for (const step of r.steps) {
      const icon = step.pass ? 'v' : 'x';
      const extra = step.screenshot ? ` [screenshot]` : '';
      console.log(`    ${icon} ${step.label}: ${step.detail}${extra}`);
    }
    console.log();
  }

  await browser.close();

  // Summary
  const pass = results.filter(r => r.pass).length;
  const fail = results.filter(r => !r.pass).length;

  console.log(`  ${'—'.repeat(50)}`);
  console.log(`  Results: ${pass} PASS, ${fail} FAIL`);
  if (fail > 0) {
    console.log('  Failed flows:');
    for (const r of results.filter(r => !r.pass)) {
      const failStep = r.steps.find(s => !s.pass);
      console.log(`    ${r.name}: step "${failStep?.label}" — ${failStep?.detail}`);
    }
  }

  // Collect all screenshots for easy viewing
  const allScreenshots = results.flatMap(r => r.screenshots);
  if (allScreenshots.length > 0) {
    console.log(`  Screenshots (${allScreenshots.length}):`);
    for (const s of allScreenshots) console.log(`    ${s}`);
  }
  console.log(`  ${'—'.repeat(50)}\n`);

  process.exit(fail > 0 ? 1 : 0);
}

main().catch(e => { console.error(`Fatal: ${e.message}`); process.exit(1); });
