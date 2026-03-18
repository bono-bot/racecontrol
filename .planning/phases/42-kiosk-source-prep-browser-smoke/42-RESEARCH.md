# Phase 42: Kiosk Source Prep + Browser Smoke — Research

**Researched:** 2026-03-19 IST
**Domain:** Next.js kiosk data-testid audit, Playwright fixture/cleanup patterns, browser smoke testing
**Confidence:** HIGH — all findings from direct source reading of live kiosk files

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FOUND-04 | Pre-test cleanup fixture — stop stale games, end billing, restart stuck agents before each test run | API endpoints confirmed: `POST /api/v1/games/stop`, `GET /api/v1/billing/active`, `POST /api/v1/billing/{id}/stop`, `GET /api/v1/games/active` |
| FOUND-06 | data-testid attributes added to kiosk wizard components for reliable Playwright selectors | Full DOM audit complete — ZERO existing data-testid attributes in any kiosk TSX file; all must be added in this phase |
| FOUND-07 | UI user navigation simulation — keyboard navigation (Tab, Enter, Escape), touch/click targets, scroll behavior | Book page uses button elements for all interactive targets — focusable by default; Tab/Enter work with no extra keyboard handling needed |
| BROW-01 | Kiosk page smoke — all pages load (200), no SSR errors, no React error boundaries | Routes confirmed: `/`, `/book`, `/control`, `/staff`, `/fleet`, `/debug`, `/pod`, `/settings`, `/spectator` |
| BROW-07 | Screenshot on failure — capture screenshot + DOM snapshot when any browser test fails for debugging | `playwright.config.ts` already has `screenshot: 'only-on-failure'` — directory config and DOM dump hook still needed |
</phase_requirements>

---

## Summary

Phase 42 has two equal-weight tasks: (1) add `data-testid` attributes to kiosk source files, and (2) write browser smoke specs. The data-testid audit reveals **zero existing attributes** across all kiosk TSX files — this is a clean slate. Every interactive element in the wizard needs a testid added in this phase so Phase 43 wizard specs can select them reliably.

The kiosk has two wizard implementations: `SetupWizard.tsx` (used by the staff control panel's pod-level wizard) and `book/page.tsx` (used by the self-service customer booking flow). Both implement identical wizard steps but as separate components. The smoke spec needs to cover the self-service booking flow at `/book` since that is the customer-facing path; the `SetupWizard.tsx` component is staff-only and covered by Phase 43.

The pre-test cleanup fixture calls three API endpoints in sequence: `GET /api/v1/billing/active` to find live sessions, `POST /api/v1/billing/{id}/stop` per session, and `POST /api/v1/games/stop` with `pod_id` for any active game. The fixture must be idempotent — calling it on a clean pod returns 200 with nothing to do.

**Primary recommendation:** Add data-testid to `kiosk/src/app/book/page.tsx` wizard steps first (customer path), then write `smoke.spec.ts` targeting all kiosk routes. SetupWizard.tsx testids can be added in the same plan since they share the same step names.

---

## data-testid Audit: Current State

### Finding: Zero existing data-testid attributes

Grep for `data-testid` across all `kiosk/src/**/*.tsx` files returns **no matches**. Every testid listed below must be added as new attributes.

### Source: book/page.tsx (customer booking flow — PRIMARY target for Phase 42 + 43)

This file renders wizard steps inline. The wizard phases are:

| Phase | Step | Container Element | Needs testid |
|-------|------|-------------------|-------------|
| `phone` | Phone number entry | `<div>` wrapping numpad | `booking-phone-screen` |
| `phone` | Phone display text | `<p>` | `phone-display` |
| `phone` | Each numpad digit button | `<button>` in grid | `numpad-digit-{n}` |
| `phone` | Send OTP button | `<button>` | `send-otp-btn` |
| `phone` | Walk-in button (staff mode only) | `<button>` | `walkin-btn` |
| `otp` | OTP entry screen | `<div>` | `booking-otp-screen` |
| `wizard` | Wizard wrapper | `<div>` | `booking-wizard` |
| `wizard/select_plan` | Plan step container | `<div>` | `step-select-plan` |
| `wizard/select_plan` | Each tier button | `<button>` | `tier-option-{tier.id}` |
| `wizard/select_game` | Game step container | `<div>` | `step-select-game` |
| `wizard/select_game` | Each game button | `<button>` | `game-option-{g.id}` |
| `wizard/select_experience` | Experience step container | `<div>` | `step-select-experience` |
| `wizard/select_experience` | Each experience button | `<button>` | `experience-option-{exp.id}` |
| `wizard/select_experience` | Custom track+car button (AC only) | `<button>` | `custom-experience-btn` |
| `wizard/review` | Review step container | `<div>` | `step-review` |
| `wizard/review` | Book button | `<button>` | `book-btn` |
| All wizard steps | Step title h2 | `<h2>` | `wizard-step-title` |
| All wizard steps | Cancel button | `<button>` | `cancel-btn` |
| `success` | Success screen | `<div>` | `booking-success` |
| `error` | Error screen | `<div>` | `booking-error` |

### Source: SetupWizard.tsx (staff pod-level wizard — covers AC-specific steps)

This component is used from `control/page.tsx` for staff pod booking. It renders all 13 AC wizard steps. The testids below are needed for Phase 43 wizard tests (but can be added in Phase 42 along with the book/ testids):

| Step | Interactive Element | Testid |
|------|---------------------|--------|
| `register_driver` | Driver search input | `driver-search` |
| `register_driver` | Each driver result button | `driver-result-{d.id}` |
| `register_driver` | Driver name input | `new-driver-name` |
| `register_driver` | Create driver continue button | `create-driver-btn` |
| `select_plan` | Tier buttons | `tier-option-{tier.id}` |
| `select_game` | Game buttons | `game-option-{g.id}` |
| `session_splits` | Split option buttons | `split-option-{opt.count}` |
| `player_mode` | Singleplayer button | `player-mode-single` |
| `player_mode` | Multiplayer button | `player-mode-multi` |
| `session_type` | Each session type button | `session-type-{type}` |
| `ai_config` | AI toggle button | `ai-toggle` |
| `ai_config` | AI difficulty buttons | `ai-difficulty-{level}` |
| `ai_config` | AI count slider | `ai-count-slider` |
| `ai_config` | Continue button | `ai-config-next` |
| `select_experience` | Preset/Custom toggle | `experience-mode-preset`, `experience-mode-custom` |
| `select_experience` | Experience list | `experience-list` |
| `select_experience` | Each experience button | `experience-option-{exp.id}` |
| `select_track` | Track search input | `track-search` |
| `select_track` | Each track button | `track-option-{t.id}` |
| `select_car` | Car search input | `car-search` |
| `select_car` | Each car button | `car-option-{c.id}` |
| `driving_settings` | Difficulty buttons | `difficulty-{key}` |
| `driving_settings` | Transmission buttons | `transmission-{key}` |
| `driving_settings` | FFB buttons | `ffb-{key}` |
| `driving_settings` | Review button | `driving-settings-next` |
| `review` | Launch button | `launch-btn` |
| All steps | Step header h3 | `wizard-step-title` |
| All steps | Back button | `wizard-back-btn` |

### Source: page.tsx (customer landing / pod grid)

The landing page is smoke-only (no wizard steps). The following testids are needed for smoke:

| Element | Testid |
|---------|--------|
| Pod grid container | `pod-grid` |
| Each idle pod card (clickable) | `pod-card-{pod.number}` |
| Book a Session footer button | `book-session-btn` |
| PIN modal overlay | `pin-modal` |
| Connection status indicator | `ws-status` |

### Source: control/page.tsx (staff control panel)

Smoke-only in Phase 42. No wizard testids needed — SetupWizard.tsx handles those.

---

## Standard Stack

### Core (already installed from Phase 41)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@playwright/test` | 1.58.2 | Browser automation, fixture system, assertions | Installed in Phase 41 |
| Playwright bundled Chromium | 1.58.2 | Real browser execution | Installed via `npx playwright install chromium` in Phase 41 |

### No new npm packages required for Phase 42

All tools needed (Playwright, TypeScript) are already installed. Phase 42 only adds:
- `data-testid` attributes to existing TSX files (source changes, no package changes)
- New spec file `tests/e2e/playwright/kiosk/smoke.spec.ts`
- New fixture file `tests/e2e/playwright/fixtures/cleanup.ts`

---

## Architecture Patterns

### Recommended Phase 42 File Layout

```
kiosk/src/
├── app/
│   ├── page.tsx              MODIFIED — add data-testid to pod grid, pod cards, CTA
│   ├── book/page.tsx         MODIFIED — add data-testid to all wizard step containers and buttons
│   └── control/page.tsx      (smoke only — no testids needed in Phase 42)
└── components/
    └── SetupWizard.tsx       MODIFIED — add data-testid to all wizard step elements

tests/e2e/
├── playwright/
│   ├── fixtures/
│   │   └── cleanup.ts        NEW — pre-test cleanup fixture
│   └── kiosk/
│       └── smoke.spec.ts     NEW — page smoke tests (BROW-01, BROW-07)
└── lib/
    └── common.sh             EXISTING (Phase 41)
```

### Pattern 1: Playwright Auto-Fixture for Cleanup

**What:** A `test.extend<{cleanup: void}>()` fixture that runs before each test, calls the cleanup logic, and yields. Tests import from `fixtures/cleanup.ts` instead of directly from `@playwright/test`.

**When to use:** Any spec that could leave stale game or billing state — which in Phase 42 is the smoke spec itself (it may encounter a live session) and all Phase 43 wizard specs.

**Example:**
```typescript
// tests/e2e/playwright/fixtures/cleanup.ts
import { test as base, request } from '@playwright/test';

const BASE_URL = process.env.KIOSK_BASE_URL ?? 'http://192.168.31.23:8080';
const TEST_POD = process.env.TEST_POD_ID ?? 'pod-8';

export const test = base.extend<{ ensureClean: void }>({
  ensureClean: [async ({}, use) => {
    // --- BEFORE: stop stale games on test pod ---
    const ctx = await request.newContext({ baseURL: BASE_URL });

    // 1. Check active games on test pod
    const gamesRes = await ctx.get('/api/v1/games/active');
    if (gamesRes.ok()) {
      const games = await gamesRes.json();
      for (const game of games ?? []) {
        if (game.pod_id === TEST_POD) {
          await ctx.post('/api/v1/games/stop', {
            data: { pod_id: TEST_POD }
          });
        }
      }
    }

    // 2. Check active billing on test pod
    const billingRes = await ctx.get('/api/v1/billing/active');
    if (billingRes.ok()) {
      const sessions = await billingRes.json();
      for (const session of sessions?.sessions ?? []) {
        if (session.pod_id === TEST_POD) {
          await ctx.post(`/api/v1/billing/${session.id}/stop`, {});
        }
      }
    }

    await ctx.dispose();
    await use(); // yield to test

    // --- AFTER: no teardown needed for smoke tests (read-only) ---
  }, { auto: true }],
});

export { expect } from '@playwright/test';
```

**Key points:**
- `auto: true` means the fixture runs for every test in files that import from this module without needing `{ensureClean}` in each test signature
- `request.newContext` targets the Axum server (:8080), not the kiosk Next.js (:3300)
- The fixture is idempotent — if no stale sessions exist, the empty loops are a no-op

### Pattern 2: Screenshot-on-Failure + DOM Snapshot

**What:** `playwright.config.ts` already sets `screenshot: 'only-on-failure'`. Screenshots land in `test-results/` by default. A `pageerror` listener attached in `beforeEach` captures uncaught JS exceptions. An `afterEach` hook optionally dumps `page.content()` to a file on failure.

**When to use:** All browser specs. Wire in smoke.spec.ts from the start.

**Example:**
```typescript
// smoke.spec.ts
import { test, expect } from '../fixtures/cleanup';

test.beforeEach(async ({ page }, testInfo) => {
  // Capture any uncaught JS errors during the test
  const jsErrors: string[] = [];
  page.on('pageerror', (err) => jsErrors.push(err.message));
  // Store for afterEach
  (testInfo as unknown as Record<string, unknown>)['_jsErrors'] = jsErrors;
});

test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status !== testInfo.expectedStatus) {
    // DOM snapshot on failure
    const dom = await page.content();
    await testInfo.attach('dom-snapshot', {
      body: Buffer.from(dom),
      contentType: 'text/html'
    });
  }
  const jsErrors = (testInfo as unknown as Record<string, unknown>)['_jsErrors'] as string[];
  if (jsErrors?.length) {
    throw new Error(`Uncaught JS errors: ${jsErrors.join('; ')}`);
  }
});
```

**Note:** `playwright.config.ts` `screenshot: 'only-on-failure'` handles PNG screenshots automatically. The DOM snapshot above adds HTML capture as an attached artifact visible in the HTML report.

### Pattern 3: Smoke Spec Route Coverage

**What:** A smoke spec that navigates to each kiosk route, waits for network idle, and asserts: (a) no pageerror events fired, (b) no React error boundary text in DOM, (c) expected structural element is visible.

**Routes to cover:**

| Route | URL | Expected Visible Element | Notes |
|-------|-----|--------------------------|-------|
| Customer landing | `/` | `[data-testid="pod-grid"]` | Pod grid renders |
| Booking page (phone step) | `/book` | `[data-testid="booking-phone-screen"]` or "Book a Session" h1 | Initial load is phone step |
| Staff login | `/staff` | Text "Staff Login" or PIN input | |
| Control panel | `/control` | Redirects to `/staff` if no session | Test after staff login or expect redirect |
| Debug | `/debug` | Any visible content, no SSR error | |
| Fleet | `/fleet` | Any visible content | |

**Note on `/control`:** This route requires `sessionStorage.kiosk_staff_name` to be set. For smoke purposes, expect either a redirect to `/staff` (also counts as "no SSR error") or pre-seed with `page.addInitScript`.

### Pattern 4: Keyboard Navigation Simulation (FOUND-07)

**What:** Tab through wizard buttons, press Enter to select, press Escape to cancel. All interactive elements in book/page.tsx are `<button>` or `<input>` elements — natively focusable, no custom keyboard handling needed.

**Example:**
```typescript
test('keyboard nav: Tab to game selector, Enter to select AC', async ({ page }) => {
  // Navigate to wizard (skip phone step via staff mode URL)
  await page.goto('/book?staff=true&pod=pod-8');
  // ...advance to select_game step...
  await page.waitForSelector('[data-testid="step-select-game"]');

  // Tab to first game option
  await page.keyboard.press('Tab');
  // The focused element should be the first game button
  const focused = await page.evaluate(() => document.activeElement?.getAttribute('data-testid'));
  expect(focused).toMatch(/game-option-/);

  // Press Enter to select it
  await page.keyboard.press('Enter');
  // Should advance to next step
  await expect(page.locator('[data-testid="wizard-step-title"]')).not.toHaveText('Select Game');
});
```

---

## Kiosk Routes Inventory

All routes discovered from `kiosk/src/app/` directory:

| Route | File | Access | Smoke priority |
|-------|------|--------|----------------|
| `/` | `app/page.tsx` | Public | HIGH — main landing |
| `/book` | `app/book/page.tsx` | Public (phone required) | HIGH — primary wizard entry |
| `/staff` | `app/staff/` | Public (PIN) | MEDIUM — staff login |
| `/control` | `app/control/page.tsx` | Auth-gated (session) | MEDIUM — redirects without session |
| `/fleet` | `app/fleet/` | Staff | LOW |
| `/debug` | `app/debug/` | Staff | LOW |
| `/pod` | `app/pod/` | Staff | LOW |
| `/settings` | `app/settings/` | Staff | LOW |
| `/spectator` | `app/spectator/` | Public | LOW |

**Phase 42 scope:** Smoke tests for `/`, `/book`, `/staff`. Other routes in Phase 43+.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Screenshot on failure | Custom screenshot wrapper | `playwright.config.ts` `screenshot: 'only-on-failure'` already configured |
| DOM snapshot attachment | Manual file write | `testInfo.attach()` — attaches HTML to the HTML report directly |
| Pre-test cleanup state | Stateful test ordering | `test.extend` fixture with `auto: true` — runs before every test automatically |
| Detecting pageerror | Polling page for error divs | `page.on('pageerror', ...)` event listener — fires on uncaught JS exceptions |
| React error boundary detection | Custom React instrumentation | Check DOM for text: `"application error"`, `"Unhandled Runtime Error"`, `"A client-side exception"` |

---

## Common Pitfalls

### Pitfall 1: book/page.tsx Has Two Wizard Implementations

**What goes wrong:** Developer adds testids only to `SetupWizard.tsx` and discovers that `/book` (the customer path) is a completely separate inline implementation in `book/page.tsx`. The smoke spec for `/book` cannot find any testid because the book page never uses `SetupWizard.tsx`.

**Why it happens:** `SetupWizard.tsx` is used by the staff control panel (`control/page.tsx` via `PodKioskView`). `book/page.tsx` re-implements all wizard steps inline for the customer self-service path. They share step names and logic via `useSetupWizard.ts` but render separately.

**How to avoid:** Add testids to BOTH `book/page.tsx` AND `SetupWizard.tsx` in the same plan. Treat them as two separate source files requiring the same testid contract.

### Pitfall 2: `/control` Smoke Test Redirects Without Staff Session

**What goes wrong:** Smoke test navigates to `/control`, expects to see pod grid, instead sees redirect to `/staff` because `sessionStorage.kiosk_staff_name` is not set.

**Why it happens:** `control/page.tsx` checks `sessionStorage.getItem("kiosk_staff_name")` in a `useEffect` and calls `router.replace("/staff")` if absent. This is client-side navigation, not HTTP redirect — the page still returns HTTP 200, but renders nothing before redirecting.

**How to avoid:** For Phase 42 smoke, assert that the page either renders without error OR performs the redirect to `/staff`. Both are valid "no SSR error" outcomes. A smoke test that navigates to `/control` and verifies no `pageerror` event fires passes even if a redirect occurs. Alternatively, pre-seed `sessionStorage` via `page.addInitScript`.

### Pitfall 3: pageerror fires BEFORE goto() returns

**What goes wrong:** A `page.on('pageerror', ...)` listener is attached after `page.goto()`. An error fires during initial page load and is missed.

**Why it happens:** The listener must be attached before navigation starts to catch errors during SSR hydration and initial render.

**How to avoid:** Attach `pageerror` listener before `page.goto()` in the `beforeEach` hook. The fixture pattern above does this correctly.

### Pitfall 4: data-testid on Non-Rendered Elements

**What goes wrong:** A testid is added to a JSX element inside a conditional block (`{step === "select_game" && ...}`). A test looks for `[data-testid="game-option-ac"]` when the page is showing the `select_plan` step. The locator returns nothing. Playwright waits for the default timeout (30s) and fails.

**Why it happens:** JSX conditionals remove elements from the DOM entirely (not just hide them). A testid on a non-current wizard step does not exist in the DOM.

**How to avoid:** When checking for step-specific elements, always wait for the step container first: `await page.waitForSelector('[data-testid="step-select-game"]')` before looking for game buttons inside it. This is also why step containers (`step-select-game`, `step-select-plan` etc.) must have testids — they are the reliable navigation anchors.

### Pitfall 5: `screenshot: 'only-on-failure'` Does Not Capture DOM

**What goes wrong:** A test fails with a JS error but the screenshot shows a blank or loading screen because the screenshot was taken before the DOM populated. The DOM snapshot is more useful than the screenshot for diagnosing React render errors.

**Why it happens:** Screenshots are pixel captures. React error boundaries may produce minimal visible output (spinner or blank). The DOM snapshot captures the full HTML including error boundary content.

**How to avoid:** Use `testInfo.attach()` with `page.content()` in `afterEach` on failure. This is complementary to the screenshot, not a replacement.

---

## Code Examples

### Smoke Spec — Verified Pattern

```typescript
// Source: Playwright official docs — page.on('pageerror'), testInfo.attach()
// tests/e2e/playwright/kiosk/smoke.spec.ts
import { test, expect } from '../fixtures/cleanup';

const ROUTES_TO_SMOKE = [
  { path: '/', name: 'customer landing', expectedSelector: 'footer' },
  { path: '/book', name: 'booking page', expectedSelector: 'h1' },
  { path: '/staff', name: 'staff login', expectedSelector: 'h2' },
];

for (const route of ROUTES_TO_SMOKE) {
  test(`smoke: ${route.name} loads without JS errors`, async ({ page }, testInfo) => {
    const jsErrors: string[] = [];
    page.on('pageerror', (err) => jsErrors.push(err.message));

    await page.goto(route.path, { waitUntil: 'networkidle' });

    // No uncaught JS exceptions
    expect(jsErrors, `JS errors on ${route.path}: ${jsErrors.join('; ')}`).toHaveLength(0);

    // No React error boundary text in DOM
    const bodyText = await page.textContent('body') ?? '';
    expect(bodyText).not.toMatch(/application error|unhandled runtime error|a client-side exception/i);

    // Expected structural element present
    await expect(page.locator(route.expectedSelector).first()).toBeVisible();

    // DOM snapshot attachment happens via afterEach (in test file header or fixture)
  });
}
```

### Cleanup Fixture — API Endpoints Verified

```typescript
// Verified API endpoints from routes.rs:
// GET  /api/v1/games/active      -> list active game states
// POST /api/v1/games/stop        -> body: { pod_id: string }
// GET  /api/v1/billing/active    -> list active billing sessions
// POST /api/v1/billing/{id}/stop -> stop a specific billing session
```

### data-testid Addition Pattern (book/page.tsx)

```tsx
// Before (no testid):
{step === "select_game" && (
  <div className="grid grid-cols-2 gap-4">
    {GAMES.map((g) => (
      <button key={g.id} onClick={() => handleSelectGame(g.id)} ...>

// After (with testids):
{step === "select_game" && (
  <div data-testid="step-select-game" className="grid grid-cols-2 gap-4">
    {GAMES.map((g) => (
      <button
        key={g.id}
        data-testid={`game-option-${g.id}`}
        onClick={() => handleSelectGame(g.id)}
        ...>
```

**Pattern rules:**
- Step containers: `data-testid="step-{step_name}"` — on the root div of each `{step === "..." && (...)}` block
- Buttons with dynamic IDs: `data-testid={`{element-type}-{id}`}` — e.g. `game-option-assetto_corsa`
- Navigation buttons: `data-testid="wizard-back-btn"`, `data-testid="cancel-btn"`, `data-testid="book-btn"`
- The wizard title h2 (in book/page.tsx): `data-testid="wizard-step-title"`
- NOTE: SetupWizard.tsx has a `<h3>` step title — same testid `wizard-step-title` for consistency

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | @playwright/test 1.58.2 |
| Config file | `playwright.config.ts` (repo root — Phase 41) |
| Quick run command | `npx playwright test tests/e2e/playwright/kiosk/smoke.spec.ts` |
| Full suite command | `npx playwright test tests/e2e/playwright/` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FOUND-04 | Pre-test cleanup stops stale games/billing before test | fixture/unit | `npx playwright test smoke.spec.ts` (fixture runs automatically) | Wave 0 |
| FOUND-06 | `[data-testid="game-option-assetto_corsa"]` is locatable in live kiosk | browser | `npx playwright test smoke.spec.ts -g "game selector"` | Wave 0 |
| FOUND-07 | Tab key navigates to game buttons; Enter selects | browser | `npx playwright test smoke.spec.ts -g "keyboard nav"` | Wave 0 |
| BROW-01 | `/`, `/book`, `/staff` load in Chromium with no pageerror | browser smoke | `npx playwright test smoke.spec.ts` | Wave 0 |
| BROW-07 | Failing test produces PNG screenshot + HTML DOM attachment | fixture/config | (verify after an intentional test failure) | Partial — screenshot config exists; DOM attachment is Wave 0 |

### Sampling Rate
- **Per task commit:** `npx playwright test tests/e2e/playwright/kiosk/smoke.spec.ts --headed=false`
- **Per wave merge:** `npx playwright test tests/e2e/playwright/`
- **Phase gate:** Smoke spec passes (all routes green, no pageerror) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/e2e/playwright/fixtures/cleanup.ts` — cleanup fixture; covers FOUND-04
- [ ] `tests/e2e/playwright/kiosk/smoke.spec.ts` — smoke spec; covers BROW-01, BROW-07, FOUND-06, FOUND-07
- [ ] `kiosk/src/app/book/page.tsx` — testid additions; covers FOUND-06
- [ ] `kiosk/src/components/SetupWizard.tsx` — testid additions; covers FOUND-06 (staff path)
- [ ] `kiosk/src/app/page.tsx` — testid additions to pod grid; covers BROW-01 selector anchor

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CSS class selectors (`div.wizard-step > button`) | `data-testid` attributes | Phase 42 (now) | Testids survive class refactors; class names are styling concerns not test contracts |
| `page.screenshot()` manual calls | `playwright.config.ts` `screenshot: 'only-on-failure'` | Phase 41 | Zero boilerplate — screenshots happen automatically |
| `globalSetup` for cleanup | `test.extend` fixture with `auto: true` | Standard Playwright pattern | Fixture is scoped to spec, not global — safer for incremental test addition |

---

## Open Questions

1. **`/control` smoke without staff session**
   - What we know: `control/page.tsx` redirects client-side to `/staff` without `sessionStorage.kiosk_staff_name`
   - What's unclear: Whether the smoke spec should pre-seed session or just assert "no SSR error" regardless of redirect
   - Recommendation: Treat redirect to `/staff` as passing for Phase 42 smoke. Document as a known redirect, not an error. Phase 43 staff tests will pre-seed session.

2. **`/book` initial state depends on `?staff=true&pod=...` params**
   - What we know: Without staff params, book page shows phone number entry. With `?staff=true`, it shows staff mode header.
   - What's unclear: Whether smoke tests should use staff mode params (to skip OTP) or test the real customer entry path
   - Recommendation: Smoke test `/book` without params (customer path, shows phone entry screen) — validates cold load. Staff mode path tested in Phase 43 BROW-04.

---

## Sources

### Primary (HIGH confidence)
- Direct read: `kiosk/src/components/SetupWizard.tsx` — full 1002-line component read; all step DOM structure confirmed
- Direct read: `kiosk/src/app/book/page.tsx` — full ~740-line booking page read; all phase/step DOM confirmed
- Direct read: `kiosk/src/app/page.tsx` — customer landing, pod grid confirmed
- Direct read: `kiosk/src/app/control/page.tsx` — staff control panel confirmed
- Direct read: `kiosk/src/hooks/useSetupWizard.ts` — wizard state machine, getFlow() logic, isAc check confirmed
- Direct read: `crates/racecontrol/src/api/routes.rs` — API endpoints for cleanup fixture verified: `/games/stop`, `/games/active`, `/billing/active`, `/billing/{id}/stop`
- Direct read: `playwright.config.ts` — Phase 41 config; `screenshot: 'only-on-failure'` confirmed, `testDir: './tests/e2e/playwright'` confirmed
- Direct grep: `data-testid` in `kiosk/src/**/*.tsx` — **zero results** — no existing testids confirmed
- Playwright official docs — `test.extend()` fixture API, `page.on('pageerror')`, `testInfo.attach()` — HIGH confidence

### Secondary (MEDIUM confidence)
- `.planning/research/STACK.md` — Playwright 1.58.2 config patterns, `baseURL: 'http://192.168.31.23:3300'` confirmed
- `.planning/research/ARCHITECTURE.md` — test directory structure, `tests/e2e/playwright/kiosk/` layout
- `.planning/research/PITFALLS.md` — Pitfall 1 (JSX crash invisible to curl), Pitfall 7 (AC wizard steps for non-AC)
- `.planning/STATE.md` — Phase 42 gate decisions, data-testid prerequisite confirmed

---

## Metadata

**Confidence breakdown:**
- data-testid audit: HIGH — direct grep of all kiosk TSX files, zero matches confirmed
- DOM structure (step containers and buttons): HIGH — full file reads of both wizard implementations
- Cleanup fixture API endpoints: HIGH — routes.rs read directly
- screenshot-on-failure: HIGH — playwright.config.ts read directly
- Keyboard navigation approach: HIGH — all interactive elements are native `<button>` elements

**Research date:** 2026-03-19 IST
**Valid until:** 2026-04-19 (stable — kiosk source only changes when features are added)
