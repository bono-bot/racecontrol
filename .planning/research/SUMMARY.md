# Project Research Summary

**Project:** RaceControl v7.0 — Comprehensive E2E Test Suite
**Domain:** Multi-layer test runner — Playwright browser automation + curl API pipeline + self-healing shell runner + deploy verification for a Rust/Axum + Next.js 16 kiosk on Windows
**Researched:** 2026-03-19 IST
**Confidence:** HIGH

## Executive Summary

RaceControl v7.0 adds a comprehensive E2E test suite to a production venue system. The current test infrastructure (three shell scripts using curl and Python) validates API endpoints and game-launch gates but is blind to browser-level failures: React render crashes, wizard step regressions, and SSR errors all produce HTTP 200 responses that slip past every existing test. The recommended approach is a two-layer suite — Playwright browser tests for the Next.js kiosk UI, curl-based shell scripts for API pipeline checks — unified under a single master entry point (`tests/e2e/run-all.sh`) with a phase-gated sequential runner that aborts on preflight failure. The recommended Playwright version is 1.58.2 using bundled Chromium (not msedge, which has a documented 30-second hang after headed tests), with `cargo nextest` replacing `cargo test` for Rust crate tests to gain per-process isolation and auto-retry.

The most important design decision is maintaining a strict boundary between what Playwright owns (browser UI flows, SSR error detection, per-game wizard step assertions) and what shell scripts own (port checks, binary verification, rc-agent remote_ops calls, deploy sequencing). Blurring this boundary — either by mocking the backend in browser tests or using Playwright for pure HTTP assertions that curl handles faster — produces a slower, less trustworthy suite. Every pitfall in this research was discovered during real test development in this session, not hypothetically: wrong API endpoint for ws_connected, AC wizard steps rendering for non-AC games, Steam dialog blocking F1 25 launch, EADDRINUSE after kiosk restart, and stale game trackers poisoning subsequent runs.

The highest-risk area is the per-game launch validation phase (Phase 3). Steam games using EA Anti-Cheat ship with a different launcher app ID than the store page (F1 25: launch ID `3059520` vs store ID `2805550`), Steam dialogs block first-run launches silently, and game state can get stuck in `Stopping` when the agent restarts mid-cleanup. These are not edge cases — they are the normal operational environment of the venue. Pre-test cleanup fixtures and stale-state detection must be built before any stateful test is written.

## Key Findings

### Recommended Stack

Playwright is the correct and only reasonable choice for browser automation in this monorepo. Cypress cannot test the racecontrol Axum API in the same test session as the kiosk UI. Jest + Puppeteer is two packages doing what Playwright does in one. The `@playwright/test` package (not the lower-level `playwright` package) provides the test runner, fixtures, API request context, trace capture, and retry logic as a unified framework.

**Core technologies:**
- `@playwright/test` 1.58.2: Browser automation + API testing — single framework for both UI and HTTP assertions, current release (Jan 2026)
- Playwright bundled Chromium (not `msedge` channel): Kiosk testing — avoids the documented 30s hang-after-headed-test bug in the msedge channel; headed mode blocked anyway in Session 0
- Bash shell runner (`tests/e2e/*.sh`): API pipeline + deploy verification — owns everything a browser cannot: port checks, PID verification, rc-agent remote_ops, binary swap validation
- `cargo nextest` 0.9.x: Rust crate test runner — per-process isolation prevents billing state leakage between tests; 3x faster parallel execution; auto-retry on flaky tests
- TypeScript 5.9.3 (reuse kiosk existing): Type-safe Playwright config and spec files — no new TS version needed

**Critical version requirements:**
- Node.js 20+ required for `@playwright/test` 1.58.2 — verify on server before installation
- `workers: 1` and `fullyParallel: false` are mandatory in playwright.config.ts — game launch tests mutate live pod state and collide if parallelized
- `reuseExistingServer: true` mandatory — venue server has kiosk already running on :3300; Playwright must attach, not restart it

### Expected Features

The existing suite (smoke.sh, game-launch.sh, cross-process.sh) covers API status codes and launch gate sequencing but has zero browser coverage. v7.0 adds the missing browser layer and unifies everything under a single entry point.

**Must have (table stakes — suite is unreliable without these):**
- Pre-test state cleanup fixture — stale games or billing sessions from prior runs corrupt subsequent tests; must run before every stateful test
- Playwright kiosk wizard browser tests — curl cannot detect React rendering errors or wizard step transitions; only a real browser catches these
- Per-step wizard assertions per game type — HTTP 200 on `/kiosk/book` does not validate wizard reaches "review"; each of the 11 wizard steps must be explicitly asserted
- Staff mode wizard test (`?staff=true&pod=pod-8`) — untested path today; skips phone/OTP entirely
- SSR pageerror detection — `page.on('pageerror')` catches uncaught JS exceptions that produce HTTP 200 + broken page
- Per-game SimType coverage (AC, F1 25, EVO, Rally, iRacing) — wizard path forks at `select_game`; each fork must be independently exercised
- Idempotent teardown — `afterEach` stops game + ends billing session regardless of test outcome
- Single master entry point `run-all.sh` — unified pass/fail exit code for pre-deploy verification
- Configurable `RC_BASE_URL` in all tests — runs against both localhost (dev) and 192.168.31.23 (venue) without code changes

**Should have (competitive — improve test quality beyond pass/fail):**
- Self-healing pre-test runner — auto-kills stale games, restarts disconnected agents, clears stuck billing; eliminates 80% of spurious failures
- Playwright trace-on-failure (`trace: 'on-first-retry'`) — full DOM + network timeline on failure; zero overhead on passing tests
- Per-game launch validation with PID check — verifies game process started on pod, not just that API returned `ok:true`
- Deploy verification script — records binary size before/after, polls `/health`, verifies `/fleet/health` shows agents reconnected
- Flaky test log — tests needing retries emit to `flaky-log.txt` for investigation; not silently passed
- Steam dialog detection (timeout-based) — flag `Launching` state persisting >60s as "Steam dialog likely blocking"

**Defer to v7.x+:**
- Test result JSON artifact for dashboard widget — future integration, not MVP
- Inactivity timer test (`page.clock.fastForward`) — real venue failure mode but medium complexity
- Auth token API test (staff terminal PIN) — security path coverage, low complexity but not blocking launch
- CI integration (cloud runner) — requires off-LAN runner; venue-only for v7.0

**Do not build (anti-features):**
- Mocking racecontrol API in Playwright tests — defeats the purpose of E2E testing; real integration bugs become invisible
- Shared billing session state across test files — creates test ordering dependencies; each test must own its session lifecycle
- Visual regression (screenshot diffing) — kiosk UI changes frequently with brand updates; constant false positives
- Parallel launch tests across multiple pods — disrupts live customer sessions; Pod 8 is the sole test target
- Continuous E2E runs every 5 minutes — tests are pre-deploy verification, not production monitoring

### Architecture Approach

The suite follows a phase-gated sequential architecture: four phases run in order under `run-all.sh`, with Phase 1 (Preflight) as a hard gate — failure aborts the run immediately. Shell scripts own HTTP-level API verification via `lib/common.sh` (shared `pass`/`fail`/`skip` functions) and `lib/pod-map.sh` (single source of truth for pod IP mapping). Playwright owns the browser layer with two projects: `chromium` for kiosk UI tests and `api` for `page.request` HTTP assertions that need cookie/session sharing. Phase 4 (Deploy Verification) uses rc-sentry :8091 as its remote exec channel — intentionally independent of racecontrol and rc-agent so it remains available even when those services are being restarted mid-test.

**Major components:**
1. `run-all.sh` — Master orchestrator: runs all 4 phases, collects exit codes, writes `results/summary.json`, exits with total failure count
2. `lib/common.sh` + `lib/pod-map.sh` — Shared shell library: eliminates the copy-paste of `pass/fail/skip` and pod IP map currently duplicated across all three existing shell scripts
3. `api/` shell scripts — Phase 2: curl-based API tests for billing lifecycle, game state, SimType parsing, launch pipeline (game-launch.sh migrated here)
4. `playwright/kiosk/` + `playwright/api/` specs — Phase 3: browser tests (wizard flows, SSR detection) and request-only API assertions
5. `lib/playwright.config.ts` — Single Playwright config: `workers: 1`, `retries: 1`, `trace: 'on-first-retry'`, `reuseExistingServer: true`, `baseURL` from `RC_BASE_URL`
6. `deploy/verify.sh` — Phase 4: binary swap, port conflict detection, service restart idempotency via rc-sentry :8091

**Build order (strict — later items depend on earlier):**
1. `lib/common.sh`, `lib/pod-map.sh` (no dependencies)
2. Refactor existing smoke.sh, cross-process.sh to source lib/common.sh
3. `api/` phase scripts (depend on lib/common.sh, lib/pod-map.sh)
4. `lib/playwright.config.ts`
5. `playwright/kiosk/smoke.spec.ts`, `wizard.spec.ts`, `playwright/api/billing.spec.ts`
6. `deploy/verify.sh` (depends on rc-sentry confirmed on pods)
7. `run-all.sh` (depends on all above)

### Critical Pitfalls

All 7 pitfalls below were observed as real failures during v7.0 test development on 2026-03-19 — not hypothetical risks.

1. **API tests pass while browser crashes (JSX render errors)** — curl returns HTTP 200 even when React crashes rendering the page. Avoid: add `page.on('pageerror')` in all Playwright browser tests; never treat API 200 as proof the kiosk renders correctly. Phase 1 must add browser smoke before any API test is considered sufficient.

2. **Wrong endpoint for ws_connected (`/pods` vs `/fleet/health`)** — `ws_connected` is in `FleetPodHealth` struct (returned by `/api/v1/fleet/health`), NOT in the pod list from `/api/v1/pods`. Any test using `/pods` for connectivity checks always shows agents as disconnected. Avoid: enforce the endpoint split with a comment in every connectivity-checking test.

3. **Steam dialogs block game launch silently** — A "Support Message" or "Product Update" dialog appears before the game process, causing rc-agent to time out waiting for the PID. Avoid: run the first launch manually on a fresh pod reboot; configure Steam offline mode on pods; add pre-launch dialog-dismissal step.

4. **Wrong Steam app ID (store ID vs EA Anti-Cheat launcher ID)** — F1 25 store ID is `2805550`; actual launch ID is `3059520` (EA Anti-Cheat bootstrapper). Using the store ID silently opens a Steam page instead of launching the game. Avoid: maintain `GAME_IDS.md` mapping store ID → launch ID; verify by running `steam://rungameid/{id}` manually.

5. **EADDRINUSE after kiosk deploy** — Killing the kiosk process leaves port 3300 in CLOSE_WAIT/TIME_WAIT; the new process fails to bind. Avoid: poll port-free status (up to 30s) before starting the new process; use `pm2 delete` + restart cycle rather than `pm2 restart`.

6. **Stale game tracker stuck in `Stopping` state** — If rc-agent restarts mid-cleanup, the in-memory game state resets but the server holds a stale `Stopping` entry. The next test's double-launch guard blocks indefinitely. Avoid: pre-flight cleanup must handle `Stopping` state explicitly; add `afterEach` teardown that verifies `/games/active` returns empty.

7. **AC wizard steps appear for non-AC games** — The `isAc` check in `useSetupWizard.ts` controls which steps appear. A bug or game ID rename can cause `select_track`/`select_car` to appear for F1 25. This is invisible to API tests. Avoid: Playwright wizard test must assert exact step sequence per game type, explicitly verifying AC-specific steps do NOT appear for Steam games.

## Implications for Roadmap

Based on research, suggested phase structure with 4 phases:

### Phase 1: Foundation + Browser Smoke

**Rationale:** Shared library must exist before any script can source it. The browser smoke layer (detecting JSX crashes) is the single highest-value addition given that all current tests are curl-based — it addresses Pitfall 1 immediately and enables the wizard tests in Phase 2. The kiosk wizard also requires `data-testid` attributes on UI elements that may not exist today; confirming this early prevents a mid-phase blocker.

**Delivers:** `lib/common.sh`, `lib/pod-map.sh`, refactored smoke.sh + cross-process.sh, `playwright.config.ts`, `playwright/kiosk/smoke.spec.ts` (SSR error detection + all routes return 200 in real browser), staff mode Playwright fixture.

**Addresses from FEATURES.md:** Pre-test state cleanup fixture, SSR pageerror detection, configurable `RC_BASE_URL`, Playwright installed and configured.

**Avoids from PITFALLS.md:** Pitfall 1 (JSX crashes invisible to curl), Pitfall 7 (wizard step correctness — smoke confirms page loads before wizard tests begin).

**Research flag:** Standard patterns — Playwright setup is well-documented; no additional phase research needed.

---

### Phase 2: API Pipeline Tests + Shared Fixtures

**Rationale:** The `api/` shell scripts and the pre-test cleanup fixture must exist before any stateful test (game launch, billing lifecycle) can run safely. game-launch.sh must be migrated to `api/launch.sh` and extended with the shared library before Phase 3 adds more stateful tests that depend on clean pod state.

**Delivers:** `api/billing.sh`, `api/simtype.sh`, `api/game-state.sh`, `api/launch.sh` (migrated game-launch.sh), pre-test cleanup fixture, stale game tracker detection + force-clear, `/fleet/health` endpoint documentation enforced in all connectivity checks.

**Addresses from FEATURES.md:** Pre-test cleanup fixture (P1), per-game SimType coverage via API, idempotent teardown.

**Avoids from PITFALLS.md:** Pitfall 2 (wrong endpoint for ws_connected), Pitfall 6 (stale game tracker in Stopping state).

**Research flag:** Standard patterns — curl API test patterns well-established; no research needed.

---

### Phase 3: Per-Game Wizard Tests + Launch Validation

**Rationale:** This is the highest-complexity and highest-value phase. Wizard tests require the kiosk to be running and Phase 1 smoke to be passing. Launch validation requires clean pod state (Phase 2 pre-flight) and verified Steam app IDs (requires manual pre-work). The AC wizard step ordering bug (Pitfall 7) can only be caught here. Steam-related pitfalls (Pitfalls 3 and 4) are only exercised here.

**Delivers:** `playwright/kiosk/wizard.spec.ts` (all 5 sim types, per-step assertions, AC-specific steps verified absent for non-AC), staff wizard test, per-game launch validation with PID polling (Pod 8), `GAME_IDS.md` documenting store vs launch IDs, Steam dialog pre-dismissal step.

**Addresses from FEATURES.md:** Kiosk wizard smoke (all games) P1, staff mode wizard test P1, per-game launch validation with PID check P1, Steam dialog detection P2.

**Avoids from PITFALLS.md:** Pitfall 3 (Steam dialogs), Pitfall 4 (wrong Steam app IDs), Pitfall 7 (AC wizard steps for non-AC games).

**Research flag:** NEEDS deeper research/validation — Steam app IDs must be verified manually for each game before specs are written; rc-sentry :8091 availability on pods must be confirmed; `data-testid` attribute presence in kiosk UI must be checked before wizard specs are scoped.

---

### Phase 4: Deploy Verification + Master Entry Point

**Rationale:** Deploy verification modifies running services (kills racecontrol, swaps binaries) and needs all other phases to be stable first — a failed deploy test that leaves services in a bad state is a worse outcome than no deploy test at all. `run-all.sh` ties everything together and can only be written once all phase scripts are finalized.

**Delivers:** `deploy/verify.sh` (binary swap, EADDRINUSE protection, port-free poll loop, agent reconnect check via `/fleet/health`), `run-all.sh` (phase-gated orchestrator, results/summary.json), Playwright HTML report configuration, total failure count exit code.

**Addresses from FEATURES.md:** Deploy verification script P1, master entry point run-all.sh P1, Playwright HTML report P1.

**Avoids from PITFALLS.md:** Pitfall 5 (EADDRINUSE after kiosk deploy — port-free polling added to verify.sh), Pitfall 2 (fleet/health used for agent reconnect check in deploy verification).

**Research flag:** Standard patterns — deploy verification is well-understood; rc-sentry usage pattern already established in codebase.

---

### Phase Ordering Rationale

- Shared library first: all shell scripts source `lib/common.sh` — it must exist before any script is written or refactored
- Browser smoke before wizard tests: confirms Playwright is installed correctly and kiosk is reachable; wizard tests have stricter prerequisites
- API fixtures before launch tests: clean pod state is a prerequisite for any stateful test; pre-flight cleanup must be proven before game launch tests depend on it
- Deploy verification last: safest order for a test that intentionally disrupts running services
- `run-all.sh` after all phases: cannot wire together phases that do not yet exist

### Research Flags

Phases needing `/gsd:research-phase` during planning:
- **Phase 3:** Steam app IDs for EA Anti-Cheat wrapped games require manual verification on the pod before writing specs. `data-testid` attribute presence in kiosk components must be audited against the actual Next.js source. rc-sentry :8091 must be confirmed deployed on all pods before Phase 4 is scoped.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Playwright setup, config, and browser smoke are thoroughly documented in official Playwright docs; no unknowns.
- **Phase 2:** curl API testing patterns are established; the main work is refactoring and migrating existing scripts, not solving novel problems.
- **Phase 4:** Deploy verification shell pattern is standard; `run-all.sh` orchestration is straightforward once phase scripts exist.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommendations sourced from official Playwright docs (1.58.2), official Next.js docs, official cargo-nextest docs. Version requirements verified against npm registry. |
| Features | HIGH | Feature list derived from existing codebase (smoke.sh, game-launch.sh, cross-process.sh, kiosk/src/app/book/page.tsx) read directly, plus official Playwright best practices docs. |
| Architecture | HIGH | Architecture grounded in the actual existing file structure read from the repo. Component boundaries derived from how the existing scripts already work, not from generic patterns. |
| Pitfalls | HIGH | All 7 pitfalls were observed as real failures during this session's test development. Zero speculative bugs — each has a corresponding real failure and a documented recovery. |

**Overall confidence:** HIGH

### Gaps to Address

- **`data-testid` attributes in kiosk UI:** wizard.spec.ts requires selector hooks on kiosk components (`sim-select`, `track-select`, `wizard-step` indicator). These must be added to the Next.js kiosk source if they do not exist today. Audit kiosk/src/app/book/ before committing to wizard spec scope.

- **rc-sentry :8091 on all pods:** deploy/verify.sh uses rc-sentry as the remote exec channel for deploy verification. If rc-sentry is not deployed on pods, Phase 4 has no remote exec channel during service restarts. Confirm deployment status before scoping Phase 4.

- **Steam dialog dismissal method:** Research identified the problem (Steam dialogs block first-run launches) and general approaches (AutoHotkey, PowerShell UIAutomation, Steam offline mode) but the specific solution for this venue's Steam configuration is not yet confirmed. This needs one manual test run on Pod 8 to confirm dialog behavior and determine the right dismissal approach.

- **Kiosk URL routing (proxy vs direct):** Research notes that kiosk is accessible both at `:3300` (direct Next.js) and via racecontrol proxy at `:8080/kiosk`. Tests should use the proxy path for consistency with venue access, but this routing must be verified working before locking in the `baseURL` in playwright.config.ts.

## Sources

### Primary (HIGH confidence)
- Playwright official docs (playwright.dev) — versions, configuration, retries, reporters, browsers, API testing, best practices
- npm registry (`@playwright/test@1.58.2`) — version confirmed current, install instructions
- Next.js official docs (nextjs.org) — `webServer` config, `reuseExistingServer`, basePath behavior
- cargo-nextest official docs (nexte.st) — process isolation model, retry, JUnit XML
- Existing codebase (read directly 2026-03-19): `tests/e2e/smoke.sh`, `tests/e2e/game-launch.sh`, `tests/e2e/cross-process.sh`, `kiosk/src/app/book/page.tsx`, `kiosk/next.config.ts`, `crates/racecontrol/src/api/routes.rs`, `crates/rc-sentry/src/main.rs`, `fleet_health.rs`, `deploy.rs`, `game_process.rs`

### Secondary (MEDIUM confidence)
- BrowserStack: Playwright Best Practices 2026, Flaky Tests in Playwright
- Evil Martians: Flaky tests relief (chronic CI retry patterns)
- Thunders AI: Modern E2E Test Architecture Patterns
- WebSearch: Playwright 1.58 features, Next.js 16 webServer config

### Tertiary (LOW confidence / validate during implementation)
- Steam dialog dismissal approaches (AutoHotkey, PowerShell UIAutomation) — not yet tested in this venue's configuration; requires manual verification on Pod 8

---
*Research completed: 2026-03-19 IST*
*Ready for roadmap: yes*
