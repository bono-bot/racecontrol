#!/usr/bin/env node
// frontend-probe.js — MI gap closure P3
//
// Headless Playwright check of critical frontend pages. Designed to run as
// an MI Tier 1 diagnostic probe (cheap, deterministic, no AI).
//
// Returns JSON to stdout for the caller (rc-watchdog or diagnostic_engine
// shelling out via Command::new):
//
//   {
//     "ok": true|false,
//     "checked_at": "2026-04-09T18:30:00+05:30",
//     "checked_from": "james_27",
//     "results": [
//       {"page": "kiosk_blanking", "url": "...", "status": "ok", "load_ms": 421},
//       {"page": "pos_billing",    "url": "...", "status": "error", "error": "Console error: ..."}
//     ]
//   }
//
// Usage:
//   node scripts/diagnostics/frontend-probe.js [--target=james_27] [--json|--pretty]
//
// Background: MI's diagnostic engine has zero frontend visibility. It checks
// process state and edge_process_count but never loads a page. 18 historical
// frontend issues (5% detection rate) all slipped through. This probe runs
// every 5 minutes and emits a DiagnosticEvent on failure.

const { chromium } = require("playwright");

const SERVER_BASE = process.env.SERVER_BASE || "http://192.168.31.23";
const TARGET_HOST = process.env.TARGET_HOST || (() => {
  const arg = process.argv.find((a) => a.startsWith("--target="));
  return arg ? arg.split("=")[1] : "james_27";
})();
const PRETTY = process.argv.includes("--pretty");

// Pages to probe — keep this list minimal. Adding pages = adding latency.
const PAGES = [
  {
    id: "kiosk_blanking",
    url: `${SERVER_BASE}:3300/kiosk/blanking`,
    waitFor: "body",
    expect_text: null, // animations may not render in headless; just check load
  },
  {
    id: "kiosk_root",
    url: `${SERVER_BASE}:3300/kiosk`,
    waitFor: "body",
    expect_text: null,
  },
  {
    id: "pos_billing",
    url: `${SERVER_BASE}:3200/billing`,
    waitFor: "body",
    expect_text: null,
  },
  {
    id: "admin_root",
    url: `${SERVER_BASE}:3201/`,
    waitFor: "body",
    expect_text: null,
  },
];

function istNow() {
  const now = new Date();
  const utcMs = now.getTime() + now.getTimezoneOffset() * 60000;
  const ist = new Date(utcMs + 5.5 * 3600 * 1000);
  return ist.toISOString().replace("Z", "+05:30");
}

async function probePage(browser, page_def) {
  const ctx = await browser.newContext({ ignoreHTTPSErrors: true });
  const page = await ctx.newPage();
  const consoleErrors = [];
  const requestFailures = [];

  page.on("console", (msg) => {
    if (msg.type() === "error") {
      consoleErrors.push(msg.text().slice(0, 200));
    }
  });
  page.on("requestfailed", (req) => {
    requestFailures.push(`${req.method()} ${req.url()} — ${req.failure()?.errorText || "unknown"}`);
  });

  const t0 = Date.now();
  let result;
  try {
    const response = await page.goto(page_def.url, {
      waitUntil: "networkidle",
      timeout: 10000,
    });
    const status_code = response ? response.status() : 0;
    const load_ms = Date.now() - t0;

    // Critical: HTTP status must be 200
    if (status_code !== 200) {
      result = {
        page: page_def.id,
        url: page_def.url,
        status: "error",
        error: `HTTP ${status_code}`,
        load_ms,
      };
    } else if (consoleErrors.length > 0) {
      result = {
        page: page_def.id,
        url: page_def.url,
        status: "error",
        error: `Console errors: ${consoleErrors.slice(0, 3).join(" | ")}`,
        console_error_count: consoleErrors.length,
        load_ms,
      };
    } else if (requestFailures.length > 0) {
      result = {
        page: page_def.id,
        url: page_def.url,
        status: "warn",
        error: `Network failures: ${requestFailures.slice(0, 3).join(" | ")}`,
        load_ms,
      };
    } else {
      result = {
        page: page_def.id,
        url: page_def.url,
        status: "ok",
        load_ms,
      };
    }
  } catch (e) {
    result = {
      page: page_def.id,
      url: page_def.url,
      status: "error",
      error: e.message.slice(0, 200),
      load_ms: Date.now() - t0,
    };
  } finally {
    await ctx.close();
  }
  return result;
}

(async () => {
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
  } catch (e) {
    console.log(JSON.stringify({
      ok: false,
      checked_at: istNow(),
      checked_from: TARGET_HOST,
      error: `Failed to launch headless browser: ${e.message}`,
      results: [],
    }));
    process.exit(2);
  }

  const results = [];
  for (const p of PAGES) {
    results.push(await probePage(browser, p));
  }
  await browser.close();

  const ok = results.every((r) => r.status === "ok");
  const output = {
    ok,
    checked_at: istNow(),
    checked_from: TARGET_HOST,
    server_base: SERVER_BASE,
    results,
  };

  if (PRETTY) {
    console.log(JSON.stringify(output, null, 2));
  } else {
    console.log(JSON.stringify(output));
  }
  // Exit code 0 = all ok, 1 = at least one page failed (for shell scripts)
  process.exit(ok ? 0 : 1);
})();
