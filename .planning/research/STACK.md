# Stack Research

**Domain:** E2E test suite — Playwright browser tests + self-healing shell runner in a Rust/Axum + Next.js 16 monorepo on Windows
**Researched:** 2026-03-19 IST
**Confidence:** HIGH (Playwright official docs + npm package registry confirmed; Next.js integration verified via official Next.js docs + vercel/next.js examples repo)

---

> **Milestone scope:** This file covers v7.0 E2E Test Suite ONLY — Playwright browser tests,
> API pipeline tests, self-healing shell runner, per-game launch validation, deploy verification.
> Existing stack (Rust/Axum, Next.js 16, rc-agent, WebSocket protocol) is NOT re-researched.
> Focus: what gets added or configured NEW for this milestone.

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `@playwright/test` | **1.58.2** | Browser automation + API testing runner for kiosk Next.js flows | 1.58.2 is the current release (January 2026). Playwright 1.57+ switched from Chromium to Chrome for Testing builds — headed tests use `chrome`, headless uses `chrome-headless-shell`. Built-in API request context (`request` fixture) means browser AND API tests run from one framework. The `webServer` config block starts the Next.js dev server automatically before tests run. |
| Playwright `chromium` channel (default) | bundled | Primary browser for kiosk UI tests | The kiosk runs in Microsoft Edge (Chromium-based). Playwright's bundled Chromium is identical-engine to Edge — no need to install browsers separately on James's machine. The `msedge` channel is available but requires system Edge and has a known 30-second hang after each headed test in 2025 builds. Use bundled Chromium for CI/automated runs; `msedge` only if testing Edge-specific behavior. |
| Bash shell runner (`tests/e2e/*.sh`) | existing | Self-healing API + deploy verification tests | Already implemented: `smoke.sh`, `game-launch.sh`, `cross-process.sh`. Pattern proven. Extend with retry wrappers and cleanup hooks rather than replacing. The shell scripts own everything that cannot be driven by a browser: port checks, PID verification, rc-agent remote_ops, binary swap validation. |
| `cargo nextest` | **0.9.x** (latest) | Rust unit + integration test runner | Replaces `cargo test` as the test runner for Rust crates. Per-process test isolation prevents state leakage between tests — critical for billing lifecycle and game state tests where global Axum state would otherwise contaminate between test cases. 3x faster parallel execution. Auto-retry on flaky tests (`--retries 2`). Outputs JUnit XML for test result aggregation. Already in the Rust ecosystem standard. |

### Supporting Libraries (npm — added to monorepo root or kiosk/)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@playwright/test` | `^1.58.2` | Test runner + assertions + API context | Always — this is the core Playwright package. Do NOT separately install `playwright` (the lower-level package) — `@playwright/test` includes everything. |
| `typescript` | `5.9.3` (already in kiosk/) | Type-safe playwright config and test files | Reuse the kiosk's existing TypeScript version. Write `playwright.config.ts` and test files in `.ts`. |
| `dotenv` | `^16.x` | Load `RC_BASE_URL`, pod IPs, auth tokens from `.env.test` | Only if test environment vars exceed what the shell scripts already export. The existing shell scripts use env vars directly — Playwright tests need the same vars passed as `process.env`. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `npx playwright install chromium` | Install Playwright's bundled Chromium browser | Run once after `npm install`. Downloads ~150MB Chrome for Testing binary to `~/.cache/ms-playwright/`. Required before first test run. Do NOT run `npx playwright install` (all browsers) — installs Firefox + WebKit unnecessarily on a Windows dev machine. |
| `npx playwright show-report` | Open the HTML report after a test run | The HTML reporter writes to `playwright-report/`. `show-report` opens it in the default browser. Use after CI failures to inspect traces, screenshots, video. |
| `npx playwright codegen http://localhost:3300/kiosk` | Record UI interactions as Playwright test code | Use during test authoring to capture wizard steps. Generates selector-stable `getByRole`, `getByText`, `getByLabel` locators. Do NOT use recorded selectors verbatim — review and use role-based locators only. |
| `cargo install cargo-nextest` | Install nextest once on James's machine | Run once. Stored in `~/.cargo/bin/`. Already in PATH per MEMORY.md Cargo PATH setup. |

---

## Installation

```bash
# From racecontrol repo root:

# 1. Install Playwright test runner
npm install -D @playwright/test@1.58.2

# 2. Install Playwright's Chromium browser (headless + headed)
npx playwright install chromium

# 3. Install cargo-nextest (one-time, for Rust crate test improvements)
export PATH="$PATH:/c/Users/bono/.cargo/bin"
cargo install cargo-nextest --locked

# No other npm packages required. dotenv is only needed if .env.test is used.
```

---

## Configuration — playwright.config.ts

Place at repo root (`racecontrol/playwright.config.ts`):

```typescript
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e/playwright',
  fullyParallel: false,           // Sequential: avoid parallel game launches on same pod
  retries: process.env.CI ? 2 : 1, // Retry once locally, twice in CI
  workers: 1,                     // Single worker: kiosk + server share one pod in tests
  reporter: [
    ['html', { open: 'never' }],  // Always write HTML report, don't auto-open
    ['junit', { outputFile: 'test-results/junit.xml' }],
  ],
  use: {
    baseURL: process.env.KIOSK_BASE_URL ?? 'http://localhost:3300',
    trace: 'on-first-retry',      // Capture trace on retry — find flaky cause
    screenshot: 'only-on-failure',
    video: 'off',                 // Video too large for 8-pod venue machine
  },
  projects: [
    {
      name: 'chromium-kiosk',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'npm run start',     // Start kiosk Next.js (already built)
    url: 'http://localhost:3300/kiosk',
    reuseExistingServer: true,    // Don't restart if already running (venue server)
    timeout: 30_000,
    cwd: './kiosk',
  },
});
```

**Key decisions in this config:**

- `fullyParallel: false` + `workers: 1` — Game launch tests mutate live pod state. Parallel workers would collide on the same pod. Sequential execution is intentional.
- `reuseExistingServer: true` — On the venue server, kiosk is already running on :3300. Playwright should attach to it, not kill and restart it.
- `retries: 1` locally — The self-healing philosophy: retry once before declaring failure. Game launch tests fail transiently (Steam dialog, PID scan delay). One retry catches these without masking real regressions.
- `trace: 'on-first-retry'` — Traces are only captured when a test fails and retries. Zero overhead on passing tests.

---

## Test Directory Structure

```
tests/
  e2e/
    smoke.sh              # existing — API smoke tests
    game-launch.sh        # existing — per-game launch gates
    cross-process.sh      # existing — schema/sync checks
    playwright/           # NEW — browser tests
      kiosk-wizard.spec.ts        # wizard flow per-game
      kiosk-smoke.spec.ts         # page load, SSR error detection
      billing-lifecycle.spec.ts   # billing start/stop via API fixture
      deploy-verify.spec.ts       # binary swap, port conflict, service health
    helpers/
      api-client.ts       # Typed wrapper around Playwright request fixture
      pod-state.ts        # Utility to query /fleet/health, /games/active
      cleanup.ts          # afterEach: stop games, end billing sessions
```

---

## Integration Points with Existing Stack

| Existing Component | How Tests Connect | Notes |
|--------------------|------------------|-------|
| racecontrol Axum server (:8080) | Playwright `request` fixture — direct HTTP to `/api/v1/*` | No mock needed. Tests hit live server. `RC_BASE_URL` env var overrides default. |
| kiosk Next.js (:3300) | Playwright `page.goto('/kiosk')` with `baseURL: http://localhost:3300` | `basePath: /kiosk` is set in `next.config.ts` — Playwright `baseURL` must be `:3300`, not `:3300/kiosk`, so that `page.goto('/kiosk')` works. |
| rc-agent remote_ops (:8090) | Shell scripts (`game-launch.sh` Gate 5) call pod IPs directly | Playwright tests do NOT call :8090 directly — they call racecontrol which relays to agents via WebSocket. |
| Shell test runner | `npm run test:shell` invokes `bash tests/e2e/smoke.sh` | Node.js `scripts` entry with `cross-env RC_BASE_URL=...`. Not managed by Playwright. |
| Rust crates (rc-common, racecontrol, rc-agent) | `cargo nextest run` in Cargo workspace | Nextest discovers all `#[test]` functions. The billing lifecycle tests in `billing.rs` benefit most from process isolation. |

---

## Self-Healing Shell Runner — Pattern

The existing shell scripts already implement self-healing patterns. The v7.0 work is to codify and extend them:

```bash
# Pattern: retry_gate <max_retries> <wait_secs> <cmd...>
retry_gate() {
  local max=$1 wait=$2; shift 2
  local attempt=0
  while [ $attempt -lt $max ]; do
    attempt=$((attempt + 1))
    if "$@"; then return 0; fi
    echo "  RETRY $attempt/$max in ${wait}s..."
    sleep "$wait"
  done
  return 1
}

# Healing: auto-cleanup stale game before launch test
heal_stale_game() {
  local pod_id=$1
  curl -s -X POST -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"$pod_id\"}" \
    "$RC_BASE_URL/games/stop" > /dev/null
  sleep 3
}

# Healing: restart rc-agent if stuck
heal_agent() {
  local pod_ip=$1
  curl -s -X POST "http://${pod_ip}:8090/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "start /MIN cmd /c C:/RacingPoint/start-rcagent.bat"}' > /dev/null
  sleep 15
}
```

These patterns already appear partially in `game-launch.sh`. The v7.0 work extracts them into a shared `helpers.sh` that all three scripts source.

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `@playwright/test` for browser tests | Cypress | Never for this project. Cypress runs tests inside the browser — no access to network layer or multi-origin requests. Can't test the racecontrol API in the same test session as the kiosk UI. Playwright's `request` fixture handles both. |
| Playwright `request` fixture for API tests | Separate `supertest`/`axios` suite | Only if you want to split API tests from browser tests. Not worth the complexity: Playwright's `request` fixture is full HTTP client with retries, auth, and assertions. One framework. |
| `cargo nextest` | `cargo test` (built-in) | Keep using `cargo test --doc` for doctests — nextest doesn't support them yet. For all other tests, nextest wins on isolation and retry. |
| Bundled Chromium | `msedge` channel | Use `msedge` only when specifically testing Edge Group Policy behavior (kiosk lockdown settings, HeadlessModeEnabled policy). For UI correctness tests, bundled Chromium is sufficient and avoids the 30s hang bug. |
| Bash shell runner | Replace with Playwright `globalSetup` | Don't replace the shell scripts with Playwright. Shell scripts own infra-level checks (binary size, port occupancy, rc-agent :8090 exec). Playwright owns browser UX. Clear boundary. |
| `workers: 1` sequential | Parallel `workers: 4` | Only safe to parallelize kiosk-smoke and billing-lifecycle tests that use `pod-99` (non-existent pod, safe to parallelize). Game launch tests on real pods must stay sequential. Use Playwright `projects` to split if needed. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `playwright` (base npm package) | The low-level package without the test runner — requires writing your own runner, assertions, retry logic. Predates `@playwright/test`. All current Playwright docs assume `@playwright/test`. | `@playwright/test` |
| `npx playwright install` (all browsers) | Downloads ~500MB of Firefox + WebKit that will never be used on this project. Kiosk runs Chromium-engine only. | `npx playwright install chromium` |
| `cypress` | Browser-only execution model: can't test Axum API in same session as kiosk UI. Also: Cypress on Windows has historically had ENOENT path issues in CI. | `@playwright/test` |
| `jest` + `puppeteer` | Two-package setup that does what Playwright does in one. Puppeteer has weaker selector stability and no built-in retry/trace. | `@playwright/test` |
| Headed mode in CI / venue tests | Headed browser requires a desktop session (Session 1 on Windows). Tests running from SSH or Task Scheduler run in Session 0 — headed will fail silently or hang. | Headless (default) for all automated runs; headed only for interactive authoring with `npx playwright codegen`. |
| `page.locator('div.wizard-step-3 > button:nth-child(2)')` CSS selectors | Brittle — breaks on any class name change or DOM reorder. | `page.getByRole('button', { name: 'Next' })`, `page.getByLabel('Car')`, `page.getByText('Confirm Booking')` — role and text-based locators survive refactors. |
| `sleep 30` hardcoded waits in shell scripts | Hides actual timing — tests pass slowly or fail intermittently when the sleep is wrong. | `wait_for_port()` + `wait_for_health()` helpers that poll with a timeout. See `game-launch.sh` pattern. |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `@playwright/test@1.58.2` | Node.js 20.x, 22.x, 24.x | Node.js version on server/.27 must be 20+. Check: `node --version`. Upgrade if needed. |
| `@playwright/test@1.58.2` | Next.js 16.1.6 (kiosk) | No version coupling — Playwright tests the kiosk over HTTP, not as a Jest transform. Any Next.js version works. |
| `@playwright/test@1.58.2` | TypeScript 5.9.3 (kiosk devDep) | Playwright 1.58 ships its own `@types/playwright` — reuse kiosk's TS 5.9.3. Add `playwright.config.ts` to `tsconfig.json` `include` array. |
| `cargo-nextest@0.9.x` | rustc 1.93.1, Cargo 1.93.1 | Compatible. Nextest tracks Cargo's MSRV. Install with `--locked` to pin resolver. |
| `@playwright/test@1.58.2` | Windows 11 Pro | Fully supported. `chrome-headless-shell` binary runs without a display. No Xvfb or virtual display needed — Chromium headless on Windows uses native Win32 APIs. |

---

## Sources

- [Playwright Release Notes](https://playwright.dev/docs/release-notes) — Version 1.58.0 released January 30 2026; 1.58.2 current patch — HIGH confidence (official Playwright docs)
- [@playwright/test npm package](https://www.npmjs.com/package/@playwright/test) — current version 1.58.2, install instructions — HIGH confidence (official npm registry)
- [Next.js Playwright Testing Guide](https://nextjs.org/docs/pages/guides/testing/playwright) — `webServer` config, `reuseExistingServer`, baseURL pattern — HIGH confidence (official Next.js docs)
- [Playwright API Testing docs](https://playwright.dev/docs/api-testing) — `request` fixture, `APIRequestContext`, combined browser+API tests — HIGH confidence (official Playwright docs)
- [Playwright Browsers docs](https://playwright.dev/docs/browsers) — bundled Chromium vs `msedge` channel, `chrome-headless-shell` in 1.57+ — HIGH confidence (official Playwright docs)
- [Playwright Test Configuration](https://playwright.dev/docs/test-configuration) — `retries`, `workers`, `fullyParallel`, `globalSetup/Teardown` — HIGH confidence (official Playwright docs)
- [Playwright Test Retries](https://playwright.dev/docs/test-retries) — retry configuration, flaky test detection — HIGH confidence (official Playwright docs)
- [Playwright Reporters](https://playwright.dev/docs/test-reporters) — HTML + JUnit XML output formats, multi-reporter config — HIGH confidence (official Playwright docs)
- [cargo-nextest homepage](https://nexte.st/) — per-process isolation model, retry support, JUnit XML output — HIGH confidence (official nextest docs)
- [msedge headed mode hang bug](https://github.com/microsoft/playwright/issues/22776) — known 30s hang after each headed msedge test — HIGH confidence (official Playwright GitHub issue tracker)
- [Playwright Windows SIGTERM note](https://playwright.dev/docs/test-global-setup-teardown) — Windows ignores SIGTERM/SIGINT in globalTeardown — HIGH confidence (official docs)
- WebSearch results: Playwright 1.58 features, Next.js 16 webServer config, cargo-nextest vs cargo test comparison — MEDIUM confidence (multiple community sources consistent with official docs)

---

*Stack research for: v7.0 E2E Test Suite — Playwright + self-healing shell runner in Rust/Next.js monorepo on Windows*
*Researched: 2026-03-19 IST*
