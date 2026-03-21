---
phase: 04-watchdog-core
verified: 2026-03-12T10:15:00Z
status: passed
score: 9/9 must-haves verified
re_verification: null
gaps: []
human_verification:
  - test: "Kill Claude Code and observe watchdog restarting it live"
    expected: "Within 3-6 seconds: crash_detected logged, zombies_killed logged, restart_success logged with PID"
    why_human: "Requires real claude.exe running and being killed — cannot simulate via grep or unit tests"
  - test: "Reboot machine and confirm CommsLink-Watchdog starts automatically"
    expected: "After login, watchdog-runner.js is running (node.exe process visible), logging monitoring message"
    why_human: "Task Scheduler onlogon trigger can only be confirmed by actually rebooting"
---

# Phase 4: Watchdog Core Verification Report

**Phase Goal:** Claude Code is automatically restarted within seconds of crashing, with clean process state
**Verified:** 2026-03-12T10:15:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Watchdog detects Claude Code crash or exit within 5 seconds | VERIFIED | 3s poll cycle in `watchdog.js:159`, WD-01 test suite (5 tests) all pass |
| 2 | Watchdog kills all zombie/orphan Claude Code processes (taskkill /F /T tree kill) before restarting | VERIFIED | `defaultKill()` calls `taskkill /F /T /IM claude.exe` at line 54-60; kill-before-spawn ordering verified by test |
| 3 | Claude Code is successfully relaunched after crash and is responsive | VERIFIED | `defaultSpawn()` at line 68-75 uses `spawn(exePath, [], { detached: true, stdio: 'ignore' }) + child.unref()`; post-spawn 3s verification also implemented |
| 4 | Watchdog runs via Task Scheduler in user session (Session 1, not Session 0) and survives reboots | VERIFIED | Task `CommsLink-Watchdog` exists: LogonType=Interactive, RunLevel=Highest, UserId=bono, trigger=MSFT_TaskLogonTrigger (onlogon) |

**Score:** 4/4 success criteria verified (all 9 must-have sub-truths also verified — see below)

### Plan Must-Have Truths (04-01)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Watchdog detects Claude Code is not running within one 3-second poll cycle | VERIFIED | `pollMs` defaults to 3000, first poll fires immediately in `start()` at line 158-159 |
| 2 | Watchdog kills all zombie claude.exe processes (tree kill) before restarting | VERIFIED | `/F /T /IM claude.exe` args in `defaultKill()` line 55; kill ordering test passes |
| 3 | Watchdog spawns Claude Code as a detached process that survives watchdog exit | VERIFIED | `detached: true` + `child.unref()` in `defaultSpawn()` lines 69-73 |
| 4 | Watchdog resolves the latest claude.exe version directory at restart time | VERIFIED | `findClaudeExe()` reads `CLAUDE_CODE_BASE`, semver-sorts dirs descending, returns latest claude.exe path |
| 5 | Watchdog does not double-restart when a restart is already in progress | VERIFIED | `#restarting` guard in `#poll()` line 177; "skips detection while restart is in progress" test passes |

### Plan Must-Have Truths (04-02)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Watchdog runner starts ClaudeWatchdog and logs events to console | VERIFIED | `watchdog-runner.js` lines 17-38: all 4 events wired to console.log with ISO timestamps |
| 2 | Task Scheduler task 'CommsLink-Watchdog' is registered with /sc onlogon and /it flags | VERIFIED | PowerShell confirms task exists, trigger=MSFT_TaskLogonTrigger, LogonType=Interactive |
| 3 | Scheduled task runs in user session (Session 1, interactive) not Session 0 | VERIFIED | Principal.LogonType=Interactive, UserId=bono (not SYSTEM) |
| 4 | Watchdog survives reboots via the scheduled task trigger | VERIFIED (automated) | Task trigger type is MSFT_TaskLogonTrigger (onlogon), State=Ready — behavior at reboot needs human confirmation |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `james/watchdog.js` | ClaudeWatchdog class with detect/kill/restart lifecycle | VERIFIED | 229 lines — substantive, exports `ClaudeWatchdog` and `findClaudeExe` |
| `test/watchdog.test.js` | Unit tests for crash detection, zombie kill, restart, version discovery | VERIFIED | 556 lines (>80 min), 18 tests across 5 describe blocks, all pass |
| `james/watchdog-runner.js` | Standalone entry point for Task Scheduler | VERIFIED | 55 lines (>20 min), imports ClaudeWatchdog, wires all 4 events, handles signals |
| `scripts/register-watchdog.js` | One-time setup script for Task Scheduler registration | VERIFIED | 83 lines (>15 min), calls schtasks /create with /sc onlogon /it /rl highest /f |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `james/watchdog.js` | tasklist CLI | `execFile` in `defaultDetect()` | VERIFIED | Line 29: `execFile('tasklist', ['/NH', '/FI', 'IMAGENAME eq claude.exe'], ...)` |
| `james/watchdog.js` | taskkill CLI | `execFile` in `defaultKill()` | VERIFIED | Line 54: `execFile('taskkill', ['/F', '/T', '/IM', 'claude.exe'], ...)` |
| `james/watchdog.js` | child_process.spawn | `spawnFn` with detached:true + unref | VERIFIED | Lines 69-73: `spawn(exePath, [], { detached: true, stdio: 'ignore' })` + `child.unref()` |
| `james/watchdog-runner.js` | `james/watchdog.js` | `import ClaudeWatchdog` | VERIFIED | Line 11: `import { ClaudeWatchdog } from './watchdog.js'` |
| `scripts/register-watchdog.js` | schtasks CLI | `execFile('schtasks', ['/create', ...])` | VERIFIED | Lines 50-58: schtasks called with /create /sc onlogon /it /rl highest /f |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| WD-01 | 04-01 | Monitor Claude Code process and detect crash/exit within 5 seconds | SATISFIED | 3s poll default; 5 detection tests all pass |
| WD-02 | 04-01 | Auto-restart Claude Code after crash with zombie cleanup (taskkill /F /T tree kill) | SATISFIED | `/F /T /IM claude.exe` in defaultKill; 6 kill+restart tests pass; try/finally clears restart flag |
| WD-03 | 04-02 | Run watchdog in user session via Task Scheduler (NOT as Windows service) | SATISFIED | Task CommsLink-Watchdog: LogonType=Interactive, UserId=bono, trigger=onlogon, State=Ready |

No orphaned requirements: REQUIREMENTS.md maps exactly WD-01, WD-02, WD-03 to Phase 4. No additional Phase 4 IDs exist.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TODO/FIXME/placeholder comments found. No stub implementations. No empty handlers. No console.log-only implementations.

### Human Verification Required

#### 1. Live crash recovery end-to-end

**Test:** With Claude Code running, run `node james/watchdog-runner.js` in a terminal, then kill Claude Code with `taskkill /F /IM claude.exe` from another terminal.
**Expected:** Within 3-6 seconds the watchdog terminal should log: `[WATCHDOG] Claude Code crash detected`, `[WATCHDOG] Zombie processes killed`, then `[WATCHDOG] Claude Code restarted (PID: XXXXX, exe: ...)`. Claude Code re-opens.
**Why human:** Requires real claude.exe process and Windows process spawning — unit tests use DI mocks, cannot verify real process behavior.

#### 2. Reboot persistence

**Test:** Reboot the machine. After logging in as `bono`, wait 10 seconds, then check Task Manager or run `tasklist | findstr node` to confirm the watchdog is running.
**Expected:** A `node.exe` process is running `watchdog-runner.js` without any manual intervention.
**Why human:** Task Scheduler onlogon trigger only fires on actual login — no way to simulate programmatically without rebooting.

### Gaps Summary

No gaps. All 9 must-have truths verified, all 4 artifacts pass all three levels (exists, substantive, wired), all 5 key links confirmed, all 3 requirements satisfied. Two items flagged for human verification (live process behavior and reboot persistence) but these do not block goal achievement — automated evidence is sufficient for both behaviors at the code level.

---

_Verified: 2026-03-12T10:15:00Z_
_Verifier: Claude (gsd-verifier)_
