# Feature Research — E2E Test Suite (v7.0)

**Domain:** End-to-end test suite for kiosk/venue-management sim racing platform
**Researched:** 2026-03-19
**Confidence:** HIGH (Playwright official docs, verified patterns from production suites, confirmed against existing codebase)

---

## Context: What Already Exists vs What This Milestone Adds

The existing test suite is entirely shell-based (curl + python). No browser automation, no wizard flow coverage, no per-game launch validation, no reusable runner structure.

### What Already Exists (Do NOT Duplicate)

| File | What It Tests | Gap It Leaves |
|------|--------------|---------------|
| `tests/e2e/smoke.sh` | 7 API endpoints — HTTP status + JSON validity | No UI, no auth flow, no wizard, no state assertions |
| `tests/e2e/game-launch.sh` | 15 gates — billing gate, double-launch guard, SimType parsing, WS connectivity, launch lifecycle | No browser, AC/F1/EVO/Rally/iRacing not individually covered, no kiosk wizard interaction |
| `tests/e2e/cross-process.sh` | Schema compat, sync table coverage, service health chains, API spot checks | No browser, no deploy lifecycle, no binary verification |

### What v7.0 Must Add

Playwright browser tests for the kiosk booking wizard (per-game flow), deploy verification (binary swap, port conflict, service restart), per-game launch validation with PID checks, a self-healing runner with pre-test cleanup, and a single master E2E entry point reusable across racecontrol, POS, and the Admin Dashboard.

---

## Feature Landscape

### Table Stakes (Tests Are Unreliable Without These)

Features where their absence makes the test suite produce false results, miss real failures, or be impossible to run reliably in the venue environment.

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| Pre-test state cleanup | Stale games or billing sessions from prior runs corrupt subsequent tests — smoke passes but launch tests fail | LOW | Pattern: before each test, call `/games/stop` on target pod + verify game state is `NONE`. game-launch.sh already does a version of this ad hoc; it needs to be a fixture. |
| Playwright for kiosk wizard browser tests | curl cannot detect React rendering errors, wizard step transitions, or component state bugs — only browser automation can | MEDIUM | Kiosk runs Next.js with SSR. Playwright is the correct tool. Use `npx playwright test` from kiosk/ with `baseURL: http://localhost:3300`. |
| Per-step wizard assertions (phone → OTP → select_plan → select_game → ... → review) | HTTP 200 on `/kiosk/book` does not validate that the wizard reaches "review" — only Playwright stepping through each phase catches regressions | MEDIUM | The wizard has 11 defined steps: `phone`, `otp`, `wizard` (multi-step: `select_plan`, `select_game`, `player_mode`, `session_type`, `ai_config`, `select_experience`, `select_track`, `select_car`, `driving_settings`, `review`). Each step must render without error and advance correctly. |
| Staff mode wizard bypass | Staff launch (`?staff=true&pod=pod-8`) skips phone/OTP — tests must cover both paths or staff launches remain untested | LOW | Pass `?staff=true&pod=pod-8` as URL params in the staff fixture. |
| SSR error detection in browser (not just HTTP status) | HTTP 200 from curl does not catch React hydration errors, missing env vars, or runtime exceptions — only a real browser catches these | LOW | Playwright `page.on('pageerror')` catches uncaught JS exceptions. Assert zero `pageerror` events during wizard walk-through. Already partially done in game-launch.sh via body string scan, but that misses client-side errors. |
| Per-game SimType test coverage (AC, F1 25, EVO, Rally, iRacing) | game-launch.sh tests `f1_25` by default and accepts `SIM_TYPE=` env var — but individual game coverage is not enforced in any test | MEDIUM | Each sim type has different wizard path (AC uses track/car picker, others use experience picker). Tests must fork at `select_game` and exercise each path. |
| Idempotent test teardown | Tests that leave billing sessions, games, or test drivers behind cause the next test run to fail differently than the first | LOW | Teardown fixture: `afterEach` stops game + ends billing if `driver_test_trial` session exists. |
| Single master script entry point | Without one, tests are scattered (smoke.sh, game-launch.sh, cross-process.sh) — no unified pass/fail for CI or pre-deploy verification | LOW | `tests/e2e/run-all.sh` or `npm run test:e2e` invoking all three existing scripts + new Playwright suite. Exit code = total failures. |
| Configurable base URL | Tests must run against both `localhost:8080` (dev) and `192.168.31.23:8080` (venue) without code changes | LOW | All existing scripts use `RC_BASE_URL` env var. Playwright config must honor the same convention via `process.env.RC_BASE_URL`. |

### Differentiators (Competitive Advantage in Test Quality)

Features that go beyond "tests pass/fail" and make the suite actively useful as a reliability tool for a live venue.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Self-healing pre-test runner | Auto-kills stale games, restarts disconnected agents, clears stuck billing — so tests reflect real system state, not prior test debris | MEDIUM | Pattern proven in game-launch.sh Gate 5: stop game → wait → if still stuck → restart agent via `/exec`. Extract to `fixtures/heal-pod.ts` or `heal-pod.sh` utility. Avoids 80% of spurious failures in practice. |
| Playwright trace-on-failure | When wizard tests fail in CI, trace.zip gives full DOM + network timeline — eliminates "works on my machine" debugging for UI regressions | LOW | Set `trace: 'on-first-retry'` in `playwright.config.ts`. Store trace artifacts with test report. Adds zero test-writing overhead. |
| Per-game launch validation with PID check | Verifies game process actually started on the pod, not just that the launch API returned `ok:true` | HIGH | Requires polling `/games/active` for `state: "running"` + non-null `pid` field. For Steam games (F1 25, EVO, Rally), PID may take 10–30s to populate — need tolerant polling, not a 2s sleep. |
| Auto-dismiss Steam dialog detection | Steam dialogs (update prompts, "game is already running") silently block game launches — detecting them via game state timeout or error message is a test differentiator | HIGH | Detection: poll game state; if `state: "launching"` persists > 60s, flag as "Steam dialog likely blocking". Auto-dismiss: out of scope for tests — this is rc-agent's responsibility. Tests should detect and report, not fix. |
| Deploy verification test | Verifies binary was actually swapped (size check), old process died, new process is serving, and port is bound — not just that the copy command ran | MEDIUM | Pattern: record binary size before deploy, run deploy, assert new size differs + `/health` responds + no CLOSE_WAIT sockets on port 8080. Windows-specific: use `/games/active` and `/fleet/health` to confirm agent reconnect. |
| Test result JSON artifact | Machine-readable `results.json` alongside human-readable output — enables Uday's dashboard to display last test run status | MEDIUM | Playwright's `--reporter=json` produces `test-results.json`. Wrap shell tests to emit a JSON summary. A single `results.json` with pass/fail counts per category feeds the fleet health dashboard. |
| Playwright HTML report | Interactive report with screenshots, video (on failure), and trace links — shareable with Uday for post-deploy verification sign-off | LOW | Playwright built-in: `--reporter=html`. Open with `npx playwright show-report`. Store as artifact after deploy runs. |
| Retry with stability tracking | Tests that fail once but pass on retry are flagged as flaky — rather than silently passing. Flaky flag triggers investigation, not just "good enough" | MEDIUM | Playwright `retries: 2` in config. Wrap results: a test that needed retries is FLAKY not PASS. Log flaky tests to a `flaky-log.txt` so they get investigated, not ignored. |
| Kiosk inactivity timer test | Verifies the 120s inactivity auto-return on phone/OTP phases works — prevents stuck kiosk screens in the venue | MEDIUM | Playwright `page.clock.fastForward(120_000)` simulates 2 minutes of inactivity. Assert router push back to `/`. This is a real venue failure mode: stuck booking screen. |
| Auth token session test | Verifies that staff terminal auth (24h session token from racecontrol.toml PIN) gates the `/terminal` endpoint correctly — tests both valid and expired token paths | MEDIUM | Use `supertest` or curl in API tests — no browser needed. Tests: valid token returns 200, wrong token returns 401, missing token returns 401. |

### Anti-Features (Explicitly Do NOT Build These)

Features that seem like obvious inclusions but would make the test suite harder to run, maintain, or trust.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Full mocking of racecontrol API in Playwright tests | "Tests should be isolated, mock the backend" | Mocking hides the real integration. The point of E2E tests is to verify the kiosk talks to racecontrol correctly. Mocking makes tests pass while real integration silently breaks. For a venue system, real API calls against a running server are the only meaningful signal. | Run Playwright against a live racecontrol instance (dev or venue). Use a `test_` prefix billing session to isolate test data without mocking. |
| Shared test state across test files | "Create one billing session, share across all tests for speed" | Test interdependency: if one test corrupts the session, all downstream tests fail with misleading errors. A test that must run after another test is not a test — it's a script. | Each Playwright test creates and tears down its own billing session via `beforeEach`/`afterEach` fixtures. Slower but reliable. |
| Visual regression (screenshot diffing) | "Catch UI regressions automatically" | The kiosk UI evolves frequently (racing red brand updates, new game additions). Screenshot diffing requires constant baseline updates and produces high false-positive rates on any intentional UI change. Every brand update becomes a test failure. | Use semantic assertions (wizard step titles render, buttons are visible by role) instead of pixel comparison. Catches functional regressions without chasing visual noise. |
| Testing on all 8 pods in parallel | "Run game launch tests on all pods simultaneously for full coverage" | Parallel launch tests on all 8 pods during a test run would disrupt live customer sessions. The venue has paying customers during operating hours. | Target Pod 8 as the canary test pod (established convention). Pod 8 is always the first to receive changes and is the test target. Never run launch tests on pods 1–7 in automated suites. |
| AI-generated test scripts | "AI can auto-generate Playwright tests from the kiosk UI" | AI-generated selectors based on current DOM will break on any component rename or UI restructure. They also generate test logic that mimics implementation, not user intent. The kiosk wizard is complex enough that generated tests miss critical state transitions (e.g., staff mode bypass, inactivity timer). | Write tests by hand against role-based selectors (`getByRole`, `getByText`). Slower to write, but each test encodes explicit intent that survives refactors. |
| Continuous E2E tests running every 5 minutes on venue server | "Always-on E2E guarantees the system is healthy" | Running game launch tests continuously disrupts Pod 8. The test suite is a pre-deploy verification tool, not a production monitor. Continuous monitoring is already handled by racecontrol's WebSocket health + email alerts. | Run E2E suite: (1) before each binary deploy, (2) after deploy to verify success, (3) on-demand via `run-all.sh`. Use existing WS health monitoring for continuous health. |
| Testing racecontrol internals (Rust unit tests as E2E) | "Add Rust tests to verify billing FSM state transitions as part of E2E" | Rust unit tests belong in `cargo test`, not in `tests/e2e/`. Mixing them inflates E2E scope and blurs the boundary between unit and integration testing. | Keep `cargo test` for Rust unit/integration tests. E2E tests verify observable behavior via HTTP API and browser UI only. |

---

## Feature Dependencies

```
[Pre-test state cleanup fixture]
    must-run-before --> [Playwright wizard tests]
    must-run-before --> [Per-game launch validation]
    must-run-before --> [Self-healing runner]

[Playwright installed in kiosk/]
    required-by --> [Kiosk wizard browser tests]
    required-by --> [SSR error detection in browser]
    required-by --> [Inactivity timer test]
    required-by --> [Per-step wizard assertions]
    required-by --> [Playwright trace-on-failure]
    required-by --> [Playwright HTML report]

[Kiosk wizard browser tests (per-step)]
    requires --> [Staff mode fixture (URL params)]
    requires --> [Pre-test state cleanup fixture]
    forks-at --> [select_game step] -- one branch per sim type

[Per-game launch validation]
    requires --> [Pre-test state cleanup fixture]
    requires --> [Active billing session on Pod 8]
    requires --> [Pod 8 agent WS connected]
    includes --> [PID polling loop with timeout]
    detects --> [Steam dialog blocking pattern]

[Active billing session fixture]
    required-by --> [Per-game launch validation]
    required-by --> [Full launch via wizard]
    creates --> [driver_test_trial session on pod-8]
    tears-down --> [billing stop afterEach]

[Single master entry point (run-all.sh)]
    invokes --> [smoke.sh]
    invokes --> [game-launch.sh]
    invokes --> [cross-process.sh]
    invokes --> [npx playwright test]
    produces --> [results.json]
    exits --> [total failure count]

[Deploy verification test]
    requires --> [Binary size before/after comparison]
    requires --> [/health poll after restart]
    requires --> [/fleet/health agent reconnect check]
    depends-on --> [racecontrol server being local or accessible]

[Test result JSON artifact]
    requires --> [Playwright --reporter=json]
    requires --> [Shell script summary wrapper]
    feeds --> [Fleet health dashboard (future)]
```

### Dependency Notes

- **Pre-test cleanup must be the first thing every test does.** game-launch.sh already demonstrates this pattern in Gate 5. It must be promoted to a shared fixture, not copy-pasted into each test file.
- **Playwright depends on the kiosk Next.js server being up.** The master script must verify kiosk responds on `:3300` (or via the `:8080` proxy) before launching any Playwright tests. If the kiosk is not running, Playwright tests abort immediately — they should SKIP, not FAIL.
- **Per-game wizard tests fork at `select_game`.** The phone auth and plan selection steps are shared. Individual game wizard paths diverge after `select_game`. The shared setup should be a Playwright fixture that lands on the `select_game` step, then each game test continues from there.
- **Launch validation with PID check requires Pod 8 to be physically accessible and WS-connected.** If Pod 8 is offline (powered down), the test must SKIP (not FAIL). The `ws_connected` field from `/fleet/health` is the gate.
- **Deploy verification test is standalone.** It does not require Playwright. It is a shell script that records binary state, triggers a deploy, and polls health. It runs after every `cargo build --release` + binary copy.

---

## MVP Definition

v7.0 "Comprehensive E2E Test Suite" is done when a single command runs all coverage and produces a unified pass/fail.

### Launch With (v7.0 MVP)

- [ ] **Playwright installed and configured** in kiosk package — `npx playwright install chromium` + `playwright.config.ts` with `baseURL`, `trace: 'on-first-retry'`, `screenshot: 'only-on-failure'`
- [ ] **Kiosk wizard smoke test (all games)** — Playwright test that walks phone→OTP→wizard per sim type (AC, F1 25, EVO, Rally, iRacing), asserts each wizard step title renders, reaches "review" without pageerror
- [ ] **Staff mode wizard test** — `?staff=true&pod=pod-8` path, verifies staff bypass lands on wizard without phone/OTP
- [ ] **Per-game launch validation** — For each sim type: create billing on Pod 8, launch via API, poll for `state: "running"` with PID, stop and clean up
- [ ] **Deploy verification script** — Records binary size, triggers restart, polls `/health` until serving, verifies `/fleet/health` shows agents reconnected
- [ ] **Pre-test cleanup fixture** — Reusable: stop any game on Pod 8, end test billing session, wait for clean state
- [ ] **Master entry point `tests/e2e/run-all.sh`** — Runs smoke.sh + cross-process.sh + game-launch.sh + `npx playwright test`, exits with total failure count
- [ ] **Playwright HTML report** — Generated in `tests/e2e/playwright-report/`, viewable after test run

### Add After Validation (v7.x)

- [ ] **Test result JSON artifact** — `results.json` with per-category pass/fail counts, feeds future dashboard widget
- [ ] **Flaky test log** — Tests that needed retries emit to `tests/e2e/flaky-log.txt` for investigation
- [ ] **Inactivity timer test** — Playwright `clock.fastForward` verifying auto-return on phone/OTP phases
- [ ] **Auth token test** — API test for staff terminal PIN: valid, invalid, expired paths

### Future Consideration (v8+)

- [ ] **Results dashboard widget in fleet health UI** — Show last E2E run status (passed/failed/when) in the kiosk control panel
- [ ] **CI integration** — Trigger `run-all.sh` on git push to main (requires CI runner with racecontrol access, not in scope for v7.0 which is venue-only)
- [ ] **Multi-pod launch parallelism test** — Verify 8 simultaneous billing starts do not corrupt each other (load test, deferred until competitive events milestone v3.0)

---

## Feature Prioritization Matrix

| Feature | Test Quality Value | Implementation Cost | Priority |
|---------|-------------------|---------------------|----------|
| Pre-test cleanup fixture | HIGH (prevents false failures) | LOW | P1 |
| Playwright kiosk wizard smoke (all games) | HIGH (covers biggest gap vs current suite) | MEDIUM | P1 |
| Staff mode wizard test | HIGH (untested path today) | LOW | P1 |
| Per-game launch validation with PID check | HIGH (verifies full pipeline per sim) | MEDIUM | P1 |
| Master entry point run-all.sh | HIGH (unifies test suite) | LOW | P1 |
| SSR pageerror detection in Playwright | HIGH (catches real breakage curl misses) | LOW | P1 |
| Playwright HTML report | MEDIUM (debug aid) | LOW | P1 |
| Playwright trace-on-failure | MEDIUM (failure diagnosis) | LOW | P1 |
| Deploy verification script | HIGH (verifies deploys succeeded) | MEDIUM | P1 |
| Retry + flaky detection | MEDIUM (improves trust) | LOW | P2 |
| Inactivity timer test | MEDIUM (real venue failure mode) | MEDIUM | P2 |
| Auth token API test | MEDIUM (security path coverage) | LOW | P2 |
| Test result JSON artifact | LOW (future dashboard integration) | MEDIUM | P3 |
| Steam dialog detection (timeout-based) | MEDIUM (real failure mode) | MEDIUM | P2 |

**Priority key:** P1 = v7.0 MVP, P2 = v7.x after validation, P3 = future milestone

---

## Implicit Requirements From Existing Code

Observations from reading the existing test files and kiosk code that constrain feature implementation:

| Observation | Implication for Feature Implementation |
|-------------|----------------------------------------|
| Kiosk wizard has 11 named steps: `phone`, `otp`, `select_plan`, `select_game`, `player_mode`, `session_type`, `ai_config`, `select_experience`, `select_track`, `select_car`, `driving_settings`, `review` | Each step needs at least one Playwright `expect` assertion. Use `STEP_TITLES` from constants as assertion targets. |
| Inactivity auto-return fires after 120s on phone/OTP phase | `page.clock.fastForward` in Playwright is the correct tool — do not `waitForTimeout(120000)` which blocks for 2 real minutes. |
| Staff mode sets `isStaffMode = searchParams.get("staff") === "true"` + `staffPodId = searchParams.get("pod")` | Playwright staff fixture: `page.goto('/kiosk/book?staff=true&pod=pod-8')`. No phone entry needed. |
| game-launch.sh uses `driver_test_trial` as the test driver ID and `tier_trial` for billing tier | Playwright billing fixtures must use same synthetic IDs. Consistent across all test files. |
| Pod IP map is hardcoded in game-launch.sh (pod_1: .89 ... pod_8: .91) | Extract to shared config `tests/e2e/pod-config.json` rather than duplicating in each test file. |
| `/fleet/health` (not `/pods`) provides `ws_connected` field | All "is pod connected?" checks must use `/fleet/health` — this is documented in game-launch.sh comments and must be enforced in the Playwright fixtures. |
| Kiosk runs on `:3300` but is also accessible via racecontrol proxy at `:8080` | Playwright `baseURL` should be the proxy URL (`http://localhost:8080`) for consistency with how venues access it, not the direct Next.js port. Test that both routes reach the same content. |
| `RC_BASE_URL` environment variable is the existing convention for all shell tests | Playwright config must read `process.env.RC_BASE_URL` and derive kiosk URL from it (`RC_BASE_URL.replace('/api/v1', '')`). |

---

## Sources

- [Playwright official: Best Practices](https://playwright.dev/docs/best-practices) — HIGH confidence
- [Playwright official: Page Object Models](https://playwright.dev/docs/pom) — HIGH confidence
- [Playwright official: Fixtures](https://playwright.dev/docs/test-fixtures) — HIGH confidence
- [Playwright official: Test Retries](https://playwright.dev/docs/test-retries) — HIGH confidence
- [Playwright official: Trace Viewer](https://playwright.dev/docs/trace-viewer) — HIGH confidence
- [BrowserStack: Playwright Best Practices 2026](https://www.browserstack.com/guide/playwright-best-practices) — MEDIUM confidence
- [BrowserStack: Flaky Tests in Playwright](https://www.browserstack.com/guide/playwright-flaky-tests) — MEDIUM confidence
- [Evil Martians: Flaky tests relief](https://evilmartians.com/chronicles/flaky-tests-be-gone-long-lasting-relief-chronic-ci-retry-irritation) — MEDIUM confidence
- [Thunders AI: Modern E2E Test Architecture](https://www.thunders.ai/articles/modern-e2e-test-architecture-patterns-and-anti-patterns-for-a-maintainable-test-suite) — MEDIUM confidence
- Existing codebase: `tests/e2e/smoke.sh`, `tests/e2e/game-launch.sh`, `tests/e2e/cross-process.sh` — HIGH confidence
- Existing codebase: `kiosk/src/app/book/page.tsx` (wizard phases, step titles, staff mode params) — HIGH confidence
- PROJECT.md v7.0 requirements (2026-03-19) — HIGH confidence

---
*Feature research for: E2E Test Suite (v7.0) — RaceControl venue-management platform*
*Researched: 2026-03-19*
