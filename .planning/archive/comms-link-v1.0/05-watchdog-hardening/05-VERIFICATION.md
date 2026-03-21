---
phase: 05-watchdog-hardening
verified: 2026-03-12T07:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 5: Watchdog Hardening Verification Report

**Phase Goal:** The watchdog handles repeated failures gracefully and re-establishes full connectivity after every restart
**Verified:** 2026-03-12T07:00:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                       | Status     | Evidence                                                                   |
|----|---------------------------------------------------------------------------------------------|------------|----------------------------------------------------------------------------|
| 1  | Repeated rapid crashes trigger escalating delays (5s/15s/30s/60s/5min) instead of thrashing | VERIFIED   | `EscalatingCooldown` in watchdog.js lines 120-168; steps array `[5000,15000,30000,60000,300000]` |
| 2  | After restart, watchdog verifies Claude Code is actually responding (not just PID alive)     | VERIFIED   | `#restart()` in watchdog.js lines 292-299: 3s post-spawn `detectFn` call, emits `self_test_passed`/`self_test_failed` |
| 3  | If self-test fails, cooldown escalates (does NOT reset to step 0)                           | VERIFIED   | `self_test_failed` handler in watchdog-runner.js line 96-98: only logs warning, no `reset()` call; Test 7 confirms |
| 4  | If self-test passes, cooldown resets to step 0                                              | VERIFIED   | `self_test_passed` handler in watchdog-runner.js line 65: `watchdog.cooldown.reset()`; Test 1 confirms |
| 5  | First crash detection has zero delay (immediate restart)                                    | VERIFIED   | `EscalatingCooldown.ready()` returns `true` when `attemptCount === 0`; `#poll()` records attempt before restart |
| 6  | After restart, WebSocket connection to Bono is re-established automatically                 | VERIFIED   | `self_test_passed` handler lines 68-70: `if (client !== null && client.state !== 'CONNECTED') client.connect()`; Tests 2 and 3 confirm |
| 7  | Bono receives an email from James after every successful restart                            | VERIFIED   | `self_test_passed` handler lines 82-91: `execFileFn('node', [sendEmailPath, 'bono@racingpoint.in', ...])` fire-and-forget; Tests 4 and 5 confirm |
| 8  | Watchdog still functions when COMMS_PSK is not set (graceful degradation)                  | VERIFIED   | Production entry point lines 130-142: `if (psk)` guard; `wireRunner` accepts `client: null`; Test 6 confirms |
| 9  | Email failure does not block the watchdog loop                                              | VERIFIED   | `execFileFn` callback only logs error (line 87-89), never throws; Test 8 confirms |

**Score:** 9/9 truths verified

---

### Required Artifacts

| Artifact                           | Expected                                                         | Status   | Details                                           |
|------------------------------------|------------------------------------------------------------------|----------|---------------------------------------------------|
| `james/watchdog.js`                | EscalatingCooldown class + modified ClaudeWatchdog with cooldown gating and self-test events; exports `EscalatingCooldown`, `ClaudeWatchdog`, `findClaudeExe` | VERIFIED | 305 lines; all three exports present; `EscalatingCooldown` class lines 120-168; cooldown gating in `#poll()` lines 250-260; self-test events in `#restart()` lines 295-299 |
| `test/watchdog.test.js`            | Unit tests for WD-04 and WD-05; contains "EscalatingCooldown"    | VERIFIED | 862 lines; describe blocks for `EscalatingCooldown (WD-04)`, `Cooldown Integration (WD-04)`, `Self-Test Events (WD-05)` all present; 14 new tests |
| `james/watchdog-runner.js`         | Integration hub: ClaudeWatchdog + CommsClient + HeartbeatSender + email notification; min 80 lines | VERIFIED | 176 lines (well above 80); `wireRunner()` exported; all four dependencies wired |
| `test/watchdog-runner.test.js`     | Unit tests for runner integration (WD-06, WD-07); contains "self_test_passed" | VERIFIED | 204 lines; 8 tests covering all required behaviors; `self_test_passed` present 8 times |

---

### Key Link Verification

| From                                    | To                                   | Via                                            | Status   | Details                                                  |
|-----------------------------------------|--------------------------------------|------------------------------------------------|----------|----------------------------------------------------------|
| `ClaudeWatchdog#poll()`                 | `EscalatingCooldown#ready()`         | `cooldown.ready()` check before restart        | WIRED    | watchdog.js line 250: `if (!this.#cooldown.ready()) return;` |
| `ClaudeWatchdog#restart()`              | `self_test_passed` / `self_test_failed` events | emit after 3s post-spawn detect verification | WIRED    | watchdog.js lines 296 and 298: both events emitted in verify branch |
| `self_test_passed` handler              | `EscalatingCooldown#reset()`         | cooldown reset on successful self-test         | WIRED    | watchdog-runner.js line 65: `watchdog.cooldown.reset()` |
| `watchdog-runner.js`                    | `CommsClient`                        | instantiation with `COMMS_PSK`/`COMMS_URL` env vars | WIRED | lines 138: `new CommsClient({ url, psk })` inside `if (psk)` guard |
| `watchdog.on('self_test_passed')`       | `commsClient.connect()`              | event handler calls connect if not connected   | WIRED    | lines 68-70: `if (client !== null && client.state !== 'CONNECTED') client.connect()` |
| `watchdog.on('self_test_passed')`       | `execFile.*send_email`               | fire-and-forget email via child_process        | WIRED    | lines 82-91: `execFileFn('node', [sendEmailPath, ...])` |
| `watchdog.on('self_test_passed')`       | `cooldown.reset()`                   | reset cooldown on successful self-test         | WIRED    | line 65: `watchdog.cooldown.reset()` — reads state first (lines 61-62), then resets |

---

### Requirements Coverage

| Requirement | Source Plan | Description                                                         | Status    | Evidence                                                    |
|-------------|-------------|---------------------------------------------------------------------|-----------|-------------------------------------------------------------|
| WD-04       | 05-01-PLAN  | Escalating cooldown on repeated crashes (5s/15s/30s/60s/5min)     | SATISFIED | `EscalatingCooldown` class in watchdog.js; 7 unit tests in `EscalatingCooldown (WD-04)` block; 2 integration tests in `Cooldown Integration (WD-04)` block |
| WD-05       | 05-01-PLAN  | Startup self-test verifies Claude Code is responding after restart  | SATISFIED | `#restart()` emits `self_test_passed`/`self_test_failed` after 3s post-spawn detect; 5 tests in `Self-Test Events (WD-05)` block |
| WD-06       | 05-02-PLAN  | Re-establish WebSocket connection to Bono after restart             | SATISFIED | `self_test_passed` handler in watchdog-runner.js calls `client.connect()` when not connected; Tests 2 and 3 in watchdog-runner.test.js |
| WD-07       | 05-02-PLAN  | Email Bono on restart: "James is back online"                       | SATISFIED | `self_test_passed` handler fires `execFileFn` with `bono@racingpoint.in` and correct subject/body; Tests 4 and 5 in watchdog-runner.test.js |

All 4 phase requirements (WD-04, WD-05, WD-06, WD-07) are satisfied. No orphaned requirements.

---

### Anti-Patterns Found

None. No TODOs, FIXMEs, placeholders, or empty implementations found in the modified files.

---

### Human Verification Required

#### 1. End-to-End Watchdog Startup Without COMMS_PSK

**Test:** Run `node james/watchdog-runner.js` without COMMS_PSK set
**Expected:** Process starts, logs "[WATCHDOG] COMMS_PSK not set -- running without WebSocket connection" and "[WATCHDOG] Monitoring Claude Code (polling every 3s)", does not crash
**Why human:** Cannot exec production runner in this environment (would attempt real tasklist/process scanning against the local machine)

#### 2. Email Body Formatting in Real Run

**Test:** Trigger a restart with a real SEND_EMAIL_PATH pointing to a stub script; inspect the email body that would be sent to bono@racingpoint.in
**Expected:** Body contains "James is back online.", ISO timestamp, attempt count, delay in ms, PID, and exe path
**Why human:** Automated test (Test 5) already verifies the body fields; human confirmation verifies the real `send_email.js` invocation path resolves correctly at runtime

---

### Test Suite Results

| Test File                        | Tests | Pass | Fail |
|----------------------------------|-------|------|------|
| test/watchdog.test.js            | 32    | 32   | 0    |
| test/watchdog-runner.test.js     | 8     | 8    | 0    |
| All test files combined          | 97    | 97   | 0    |

---

### Summary

Phase 5 goal is fully achieved. All 9 observable truths verified. All 4 artifacts exist and are substantive (no stubs). All 7 key links confirmed wired in actual source code.

**WD-04 (escalating cooldown):** `EscalatingCooldown` class is a proper implementation with private fields, correct step array `[5000, 15000, 30000, 60000, 300000]`, clamping at the last step, and `ready()`/`recordAttempt()`/`reset()` API. Injected into `ClaudeWatchdog` via constructor DI and exposed via getter. Poll loop gates on `cooldown.ready()` and records attempt before restart.

**WD-05 (self-test):** `#restart()` waits 3 seconds post-spawn, re-runs `detectFn`, and emits `self_test_passed` or `self_test_failed` (alongside backward-compatible `restart_failed`). Cooldown is not reset inside `ClaudeWatchdog` — consumer owns reset policy.

**WD-06 (WebSocket re-establishment):** `wireRunner`'s `self_test_passed` handler calls `client.connect()` when client exists and is not already `CONNECTED`. Gracefully skips when `client` is `null` (no PSK).

**WD-07 (email notification):** `wireRunner`'s `self_test_passed` handler reads cooldown state before reset, builds a body with timestamp/attempt count/delay/PID/exe path, and fires `execFileFn` as fire-and-forget. Errors are logged but never thrown.

The two human verification items are confirmations of runtime behavior, not gaps. The automated test suite (97/97 passing) provides complete coverage of the wiring logic.

---

_Verified: 2026-03-12T07:00:00Z_
_Verifier: Claude (gsd-verifier)_
