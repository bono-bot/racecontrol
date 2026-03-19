# Requirements: RaceControl v7.0 E2E Test Suite

**Defined:** 2026-03-19
**Core Value:** Comprehensive, self-healing E2E test coverage for the full kiosk→server→agent→game launch pipeline

## v7.0 Requirements

### Foundation

- [x] **FOUND-01**: Shared shell library (`lib/common.sh`) with pass/fail/skip/info helpers and exit code tracking
- [x] **FOUND-02**: Shared pod IP map (`lib/pod-map.sh`) with all 8 pod IPs, used by all test scripts
- [x] **FOUND-03**: Playwright installed with `playwright.config.ts` — bundled Chromium, `reuseExistingServer`, sequential workers
- [x] **FOUND-04**: Pre-test cleanup fixture — stop stale games, end billing, restart stuck agents before each test run
- [x] **FOUND-05**: cargo-nextest configured for Rust crate tests with per-process isolation and built-in retries
- [x] **FOUND-06**: data-testid attributes added to kiosk wizard components for reliable Playwright selectors
- [x] **FOUND-07**: UI user navigation simulation — keyboard navigation (Tab, Enter, Escape), touch/click targets, scroll behavior

### Browser Tests

- [x] **BROW-01**: Kiosk page smoke — all pages load (200), no SSR errors, no React error boundaries
- [x] **BROW-02**: AC wizard flow — full 13-step flow with track/car selection, AI config, driving settings
- [x] **BROW-03**: Non-AC wizard flow — simplified 5-step flow (game → experience → review) for F1 25, EVO, Rally, iRacing
- [x] **BROW-04**: Staff mode booking — `?staff=true&pod=pod-8` bypass path tested end-to-end
- [x] **BROW-05**: Experience filtering — only selected game's experiences appear, Custom button hidden for non-AC
- [x] **BROW-06**: UI navigation — page transitions, back/forward, step indicators update correctly
- [x] **BROW-07**: Screenshot on failure — capture screenshot + DOM snapshot when any browser test fails for debugging

### API & Launch

- [x] **API-01**: Billing gates — reject launch without billing, create/end session, timer sync
- [x] **API-02**: Per-game launch — launch each installed game (AC, F1 25, EVO, Rally, iRacing), verify PID or Launching state
- [x] **API-03**: Game state lifecycle — Idle→Launching→Running→Stop→Idle, timeout at 60s, auto-relaunch on crash
- [x] **API-04**: Steam dialog auto-dismiss — close "Support Message" windows via WM_CLOSE during launch tests
- [x] **API-05**: Error window screenshot — capture screenshots of unexpected popup/error windows on pods for AI debugger analysis

### Deploy & Orchestration

- [x] **DEPL-01**: Deploy verification — binary swap check, port conflict detection (EADDRINUSE), service restart health
- [x] **DEPL-02**: Fleet health validation — all 8 pods WS connected, correct build_id, installed_games match config
- [x] **DEPL-03**: Master `run-all.sh` — phase-gated orchestrator with exit code collection and summary report
- [x] **DEPL-04**: AI debugger error logging — route test failures and error screenshots to AI debugger for automated analysis

## v8.0 Requirements (Phase 50: LLM Self-Test + Fleet Health)

### Self-Test Probes

- **SELFTEST-01**: self_test.rs module with 18 deterministic probes (WS, lock screen, remote ops, overlay, debug server, 5 UDP ports, HID, Ollama, CLOSE_WAIT, single instance, disk, memory, shader cache, build_id, billing state, session ID, GPU temp, Steam) — each probe returns pass/fail/skip with detail string, 10s timeout per probe
- **SELFTEST-02**: Local LLM verdict generation — feed all 18 probe results to rp-debug model, return HEALTHY/DEGRADED/CRITICAL with correlation analysis linking related failures and auto-fix recommendations
- **SELFTEST-03**: Server endpoint `GET /api/v1/pods/{id}/self-test` — triggers self-test on target pod via WebSocket command, returns full probe results + LLM verdict within 30s
- **SELFTEST-04**: Expanded auto-fix patterns 8-14 in ai_debugger.rs — DirectX (shader cache clear + device reset), memory (process trim), DLL (sfc scan), Steam (restart), performance (power plan), network (adapter reset)
- **SELFTEST-05**: E2E test `tests/e2e/fleet/pod-health.sh` — trigger self-test on all 8 pods via API, assert all HEALTHY, wired into run-all.sh as final phase gate
- **SELFTEST-06**: Self-test runs at rc-agent startup (post-boot verification) and on-demand via server command — startup results included in BootVerification message

## Future Requirements

### v7.x (after core suite validated)

- **FLAKY-01**: Flaky test detection — track pass/fail history, flag inconsistent tests
- **TIMER-01**: Inactivity timer test — verify billing pauses when AC STATUS=PAUSE
- **AUTH-01**: Auth token lifecycle test — validate JWT expiry, refresh, and session persistence
- **PERF-01**: Performance benchmarks — page load times, API response times under load

## Out of Scope

| Feature | Reason |
|---------|--------|
| Visual regression screenshots | Kiosk UI changes too frequently — maintenance overhead exceeds value |
| Full API mocking | Tests must run against live venue infrastructure, not simulated |
| Mobile/responsive testing | Kiosk runs fullscreen on fixed displays — no mobile viewport needed |
| Load/stress testing | Venue has exactly 8 pods — concurrency is fixed, not variable |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 41 | Complete |
| FOUND-02 | Phase 41 | Complete |
| FOUND-03 | Phase 41 | Complete |
| FOUND-05 | Phase 41 | Complete |
| FOUND-04 | Phase 42 | Complete |
| FOUND-06 | Phase 42 | Complete |
| FOUND-07 | Phase 42 | Complete |
| BROW-01 | Phase 42 | Complete |
| BROW-07 | Phase 42 | Complete |
| BROW-02 | Phase 43 | Complete |
| BROW-03 | Phase 43 | Complete |
| BROW-04 | Phase 43 | Complete |
| BROW-05 | Phase 43 | Complete |
| BROW-06 | Phase 43 | Complete |
| API-01 | Phase 43 | Complete |
| API-02 | Phase 43 | Complete |
| API-03 | Phase 43 | Complete |
| API-04 | Phase 43 | Complete |
| API-05 | Phase 43 | Complete |
| DEPL-01 | Phase 44 | Complete |
| DEPL-02 | Phase 44 | Complete |
| DEPL-03 | Phase 44 | Complete |
| DEPL-04 | Phase 44 | Complete |

| SELFTEST-01 | Phase 50 | Complete |
| SELFTEST-02 | Phase 50 | Complete |
| SELFTEST-03 | Phase 50 | Complete |
| SELFTEST-04 | Phase 50 | Complete |
| SELFTEST-05 | Phase 50 | Complete |
| SELFTEST-06 | Phase 50 | Complete |

**Coverage:**
- v7.0 requirements: 23 total (all complete)
- v8.0 requirements: 6 total (Phase 50)
- Mapped to phases: 29
- Unmapped: 0

---
*Requirements defined: 2026-03-19*
*Last updated: 2026-03-19 — traceability mapped to phases 41–44, 50*
