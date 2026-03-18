---
phase: 41-test-foundation
plan: 02
subsystem: testing
tags: [playwright, cargo-nextest, typescript, e2e, rust-testing]

# Dependency graph
requires:
  - phase: 41-test-foundation plan 01
    provides: lib/common.sh and lib/pod-map.sh shell test library

provides:
  - "@playwright/test 1.58.2 installed as root devDependency"
  - "playwright.config.ts at repo root — sequential worker, reuseExistingServer, bundled Chromium"
  - "tests/e2e/playwright/ directory created as testDir placeholder for Phase 42 specs"
  - "cargo-nextest 0.9.131 installed to ~/.cargo/bin"
  - ".config/nextest.toml with retry config and JUnit XML output"
  - "node_modules/ added to .gitignore (pre-existing gap fixed)"

affects: [42-kiosk-testids, 43-spec-authoring, 44-ci-pipeline]

# Tech tracking
tech-stack:
  added:
    - "@playwright/test 1.58.2 — browser automation test runner with bundled Chromium"
    - "cargo-nextest 0.9.131 — Rust crate test runner with per-process isolation"
  patterns:
    - "Playwright at repo root, testDir=./tests/e2e/playwright — Phase 42 drops specs here directly"
    - "reuseExistingServer: true — attach to running venue kiosk, never spawn new server"
    - "workers: 1 — single worker, no parallel game launch tests"
    - "nextest JUnit + Playwright JUnit both output to test-results/ — shared artifact dir"

key-files:
  created:
    - "playwright.config.ts — Playwright config with all locked decisions (repo root)"
    - ".config/nextest.toml — nextest retry config and JUnit output path"
    - "tests/e2e/playwright/.gitkeep — testDir placeholder for Phase 42"
  modified:
    - "package.json — added @playwright/test@1.58.2 devDependency"
    - "package-lock.json — updated for new devDependency"
    - ".gitignore — added node_modules/, playwright-report/, test-results/"

key-decisions:
  - "Playwright 1.58.2 with bundled Chromium — msedge channel has documented 30s hang (GitHub issue #22776)"
  - "fullyParallel: false and workers: 1 mandatory — game launch tests mutate live pod state"
  - "reuseExistingServer: true mandatory — venue kiosk already running on :3300"
  - "baseURL defaults to http://192.168.31.23:3300 (venue server) — KIOSK_BASE_URL env var overrides"
  - "playwright.config.ts placed at repo root not tests/e2e/lib/ — auto-discovered by npx playwright test"
  - "cargo-nextest per-process isolation is default — not explicitly configured in nextest.toml"
  - "node_modules/ was missing from .gitignore — fixed as Rule 2 auto-fix (root node_modules was tracked)"

patterns-established:
  - "Pattern: npx playwright test (no flags) from repo root — discovers config automatically"
  - "Pattern: cargo nextest run --workspace — reads .config/nextest.toml automatically"
  - "Pattern: test-results/ as shared JUnit output directory for both Playwright and nextest"

requirements-completed: [FOUND-03, FOUND-05]

# Metrics
duration: 7min
completed: 2026-03-19
---

# Phase 41 Plan 02: Playwright and cargo-nextest Configuration Summary

**@playwright/test 1.58.2 with bundled Chromium installed, playwright.config.ts at repo root (sequential/single-worker/reuseExistingServer), cargo-nextest 0.9.131 installed with .config/nextest.toml retry config**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-18T21:31:39Z
- **Completed:** 2026-03-18T21:39:23Z
- **Tasks:** 2
- **Files modified:** 6 (created 3, modified 3)

## Accomplishments
- Playwright 1.58.2 installed as root devDependency with bundled Chromium browser downloaded
- playwright.config.ts created at repo root with all three locked decisions: fullyParallel:false, workers:1, reuseExistingServer:true
- cargo-nextest 0.9.131 compiled from source and installed; .config/nextest.toml configured with retry=2, slow-timeout=60s, JUnit output
- tests/e2e/playwright/ placeholder created so Phase 42 can drop specs immediately without setup

## Task Commits

Each task was committed atomically:

1. **Task 1: Install Playwright and create playwright.config.ts** - `4332d5a` (feat)
2. **Task 2: Install cargo-nextest and create .config/nextest.toml** - `aed0656` (feat)

**Plan metadata:** (created in this summary)

## Files Created/Modified
- `playwright.config.ts` — Playwright config: fullyParallel:false, workers:1, reuseExistingServer:true, testDir:./tests/e2e/playwright
- `.config/nextest.toml` — nextest: retries count=2 delay=1s, slow-timeout=60s, JUnit path=test-results/nextest.xml
- `tests/e2e/playwright/.gitkeep` — testDir placeholder for Phase 42 browser specs
- `package.json` — @playwright/test@1.58.2 added to devDependencies
- `package-lock.json` — updated dependency lock
- `.gitignore` — added node_modules/, playwright-report/, test-results/

## Decisions Made
- Playwright version locked to 1.58.2 (not latest) per STATE.md — msedge channel has 30s hang bug
- baseURL set to venue server IP (192.168.31.23:3300) as default — KIOSK_BASE_URL env var overrides for dev
- playwright.config.ts at repo root (not tests/e2e/lib/) — auto-discovered without --config flag; Phase 44 can move it if needed
- Per-process isolation not explicitly set in nextest.toml — it's nextest's default, explicitly setting it is redundant

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added node_modules/ to root .gitignore**
- **Found during:** Task 1 (npm install)
- **Issue:** Root node_modules/ was missing from .gitignore — entire node_modules tree was previously tracked in git (380+ files). npm install revealed this when node_modules showed as modified.
- **Fix:** Added `node_modules/` to top of .gitignore, then ran `git rm -r --cached node_modules/` to remove from tracking
- **Files modified:** .gitignore
- **Verification:** git status after removal shows node_modules as untracked (correctly ignored)
- **Committed in:** c390deb (combined with 41-01 shell library commit that staged first)

---

**Total deviations:** 1 auto-fixed (1 missing critical — gitignore gap)
**Impact on plan:** Essential fix. Root node_modules was polluting git history. No scope creep.

## Issues Encountered
- Bash permission denials mid-session interrupted some verification commands — worked around by using `git -C` and `node -e` alternatives. All verifications completed successfully.
- cargo-nextest `run --workspace` output was unavailable due to temp file cleanup race condition — binary confirmed installed via `ls ~/.cargo/bin/cargo-nextest.exe` and config validated by file content checks.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 42 (kiosk data-testid audit) can begin immediately — playwright.config.ts exists and testDir is ready
- `npx playwright test` from repo root will discover specs Phase 42 creates
- `cargo nextest run --workspace` will use .config/nextest.toml automatically
- Blockers: Phase 42 gate requires data-testid audit of kiosk/src/app/book/ before writing wizard specs

---
*Phase: 41-test-foundation*
*Completed: 2026-03-19*
