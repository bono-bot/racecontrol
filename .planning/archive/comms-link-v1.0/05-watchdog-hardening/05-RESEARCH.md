# Phase 5: Watchdog Hardening - Research

**Researched:** 2026-03-12
**Domain:** Node.js process supervision, escalating backoff, WebSocket lifecycle management, email notification
**Confidence:** HIGH

## Summary

Phase 5 hardens the existing ClaudeWatchdog (watchdog.js) and watchdog-runner.js with four capabilities: escalating cooldown on repeated crashes (WD-04), post-restart self-test (WD-05), WebSocket re-establishment to Bono (WD-06), and email notification on successful restart (WD-07). All four capabilities build on well-understood Node.js patterns and the existing codebase already provides clean integration points.

The codebase is pure ESM (Node.js v22.14.0) with zero external dependencies beyond `ws`. Tests use `node:test` built-in runner with `t.mock.timers` for timer control. The existing ClaudeWatchdog is an EventEmitter with full DI, emitting `crash_detected`, `zombies_killed`, `restart_success`, and `restart_failed` events. The watchdog-runner.js is a thin entry point that wires logging to those events. Both index.js (the main comms entry point) and watchdog-runner.js are independent scripts -- the runner currently has NO WebSocket or heartbeat wiring.

**Primary recommendation:** Create a standalone `EscalatingCooldown` class (pure state machine, no timers), integrate it into ClaudeWatchdog's poll loop, then extend watchdog-runner.js to own a CommsClient + HeartbeatSender alongside the watchdog, wiring restart events to WebSocket reconnect and email notification.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Escalating cooldown steps: 5s, 15s, 30s, 60s, 5min -- clamped at last step for attempts beyond step count
- Reset cooldown to step 0 when Claude Code is confirmed running after restart
- Watchdog poll loop checks cooldown.ready() before attempting restart
- Self-test: re-run detect() after short delay to confirm process did not immediately die
- If self-test fails, do NOT reset cooldown -- let it escalate
- If self-test passes, reset cooldown to step 0
- watchdog-runner.js creates and owns CommsClient instance alongside ClaudeWatchdog
- On restart_success + self-test pass, call commsClient.connect() if not already connected
- HeartbeatSender wired to CommsClient (start on 'open', stop on 'close')
- Email via send_email.js using execFile (not exec -- avoids shell injection)
- Email body includes: restart timestamp, attempt count, cooldown step, Claude Code exe path
- Fire-and-forget: don't block watchdog loop on email delivery
- No rate limiting on email -- only sent on successful restart

### Claude's Discretion
- Exact class structure for EscalatingCooldown (standalone class or integrated into ClaudeWatchdog)
- Whether to add cooldown/email as constructor options or hardcode
- Test structure (extend existing watchdog tests or create new test file)

### Deferred Ideas (OUT OF SCOPE)
- WhatsApp notification to Uday on crash/recovery -- Phase 6 (Alerting)
- Daily health summary -- Phase 8
- LOGBOOK sync after restart -- Phase 7
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WD-04 | Escalating cooldown on repeated crashes (5s -> 15s -> 30s -> 60s -> 5min) | EscalatingCooldown class with steps array, ready()/recordAttempt()/reset() methods; integrated into ClaudeWatchdog poll gate |
| WD-05 | Startup self-test verifies Claude Code is responding after restart | Extend existing 3s post-spawn detection: if process_died_after_spawn, do NOT reset cooldown; if verify passes, reset cooldown and emit self_test_passed |
| WD-06 | Re-establish WebSocket connection to Bono after restart | watchdog-runner.js instantiates CommsClient + HeartbeatSender; on restart_success + self-test pass, calls commsClient.connect() |
| WD-07 | Email Bono on restart: "James is back online" | execFile('node', [send_email_path, to, subject, body]) fire-and-forget after self-test pass |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:events | v22.14.0 built-in | EventEmitter base for ClaudeWatchdog | Already in use, zero deps |
| node:child_process | v22.14.0 built-in | execFile for send_email.js invocation | Already used for tasklist/taskkill/spawn |
| node:test | v22.14.0 built-in | Test runner with mock.timers | Already used across all 75 tests |
| ws | ^8.19.0 | WebSocket client (via CommsClient) | Already a dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| node:assert/strict | built-in | Test assertions | All test files |
| node:timers | built-in | setTimeout/setInterval (mocked in tests) | Timer-dependent logic |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Standalone EscalatingCooldown class | Inline cooldown logic in ClaudeWatchdog | Standalone class is testable in isolation, follows DI pattern of the codebase |
| execFile for email | exec for email | exec spawns a shell (shell injection risk); execFile is safer and already the project pattern |
| Fire-and-forget email | await email send | Blocking watchdog loop on email is a reliability risk -- email failure should not delay restart recovery |

**Installation:**
```bash
# No new dependencies needed -- everything is built-in or already installed
```

## Architecture Patterns

### Recommended Project Structure
```
james/
  watchdog.js           # ClaudeWatchdog + EscalatingCooldown (add cooldown class here)
  watchdog-runner.js    # Entry point: extend with CommsClient + HeartbeatSender + email
  comms-client.js       # Existing -- used by runner (unchanged)
  heartbeat-sender.js   # Existing -- used by runner (unchanged)
  system-metrics.js     # Existing -- used by heartbeat (unchanged)
  index.js              # Existing main entry -- NOT modified (separate from runner)
test/
  watchdog.test.js      # Extend with WD-04, WD-05, WD-06, WD-07 tests
```

### Pattern 1: EscalatingCooldown State Machine
**What:** A pure state machine that tracks attempt count, computes delay, and reports readiness. No timers -- the poll loop calls `ready()` which compares `Date.now()` against `lastAttemptTime + currentDelay`.
**When to use:** Any time you need escalating backoff that is testable without timer mocking.
**Example:**
```javascript
// Standalone class, exported from watchdog.js
export class EscalatingCooldown {
  #steps;
  #attemptCount = 0;
  #lastAttemptTime = 0;
  #nowFn;

  constructor({ steps = [5000, 15000, 30000, 60000, 300000], nowFn = Date.now } = {}) {
    this.#steps = steps;
    this.#nowFn = nowFn;
  }

  /** Current delay in ms (clamped to last step). */
  get delay() {
    if (this.#attemptCount === 0) return 0;
    const idx = Math.min(this.#attemptCount - 1, this.#steps.length - 1);
    return this.#steps[idx];
  }

  /** Current attempt count (for email body). */
  get attemptCount() { return this.#attemptCount; }

  /** Whether enough time has elapsed since last attempt. */
  ready() {
    if (this.#attemptCount === 0) return true;
    return (this.#nowFn() - this.#lastAttemptTime) >= this.delay;
  }

  /** Record that a restart attempt is being made. */
  recordAttempt() {
    this.#attemptCount++;
    this.#lastAttemptTime = this.#nowFn();
  }

  /** Reset after successful recovery. */
  reset() {
    this.#attemptCount = 0;
    this.#lastAttemptTime = 0;
  }
}
```

**Key design choice:** Inject `nowFn` (defaults to `Date.now`) for testability. Tests pass a fake clock function -- no need for `t.mock.timers` to test cooldown logic itself. This is simpler and more reliable than mocking timers.

### Pattern 2: Cooldown Integration into Poll Loop
**What:** The poll method checks `cooldown.ready()` before calling `#restart()`. On crash detection, calls `cooldown.recordAttempt()`. On successful self-test, calls `cooldown.reset()`.
**When to use:** Gating restart attempts without adding timer complexity.
**Example:**
```javascript
// In ClaudeWatchdog#poll():
async #poll() {
  if (this.#restarting) return;
  if (!this.#cooldown.ready()) return;  // <-- NEW: skip if in cooldown

  const running = await this.#detectFn();
  if (running === false) {
    this.#cooldown.recordAttempt();  // <-- NEW: record before restart
    await this.#restart();
  }
}
```

### Pattern 3: Self-Test as Part of Restart Flow
**What:** The existing 3s post-spawn verify in `#restart()` already checks if the process died immediately. This IS the self-test (WD-05). The enhancement is: emit a `self_test_passed` or `self_test_failed` event, and only reset cooldown on pass.
**When to use:** Distinguishing "process spawned" from "process is actually running."
**Example:**
```javascript
// In ClaudeWatchdog#restart(), after the 3s verify:
const verified = await this.#detectFn();
if (verified === true) {
  this.emit('self_test_passed', { pid, exePath, timestamp: Date.now() });
  // Cooldown reset happens in the event handler or here
} else {
  this.emit('restart_failed', { reason: 'process_died_after_spawn', timestamp: Date.now() });
  this.emit('self_test_failed', { reason: 'process_died_after_spawn', timestamp: Date.now() });
  // Do NOT reset cooldown -- escalate
}
```

### Pattern 4: Runner as Integration Hub
**What:** watchdog-runner.js becomes the integration hub that wires ClaudeWatchdog + CommsClient + HeartbeatSender + email notification. Mirrors index.js pattern for CommsClient/HeartbeatSender wiring.
**When to use:** When the runner is the single entry point for the watchdog process.
**Example:**
```javascript
// watchdog-runner.js (extended)
import { ClaudeWatchdog } from './watchdog.js';
import { CommsClient } from './comms-client.js';
import { HeartbeatSender } from './heartbeat-sender.js';
import { execFile } from 'node:child_process';

const SEND_EMAIL_PATH = 'C:/Users/bono/racingpoint/racecontrol/send_email.js';

const psk = process.env.COMMS_PSK;
const url = process.env.COMMS_URL || 'ws://localhost:8765';

const watchdog = new ClaudeWatchdog();

// Only create comms if PSK is available (graceful degradation)
const client = psk ? new CommsClient({ url, psk }) : null;
const heartbeat = client ? new HeartbeatSender(client) : null;

if (client) {
  client.on('open', () => heartbeat.start());
  client.on('close', () => heartbeat.stop());
  client.connect();
}

watchdog.on('self_test_passed', ({ pid, exePath, timestamp }) => {
  // Re-establish WebSocket (WD-06)
  if (client && client.state !== 'CONNECTED') {
    client.connect();
  }

  // Email notification (WD-07)
  const body = `James is back online.\nRestart at: ${new Date(timestamp).toISOString()}\nPID: ${pid}\nExe: ${exePath}`;
  execFile('node', [SEND_EMAIL_PATH, 'bono@racingpoint.in', 'James is back online', body],
    { timeout: 30000 },
    (err) => { if (err) console.error('[WATCHDOG] Email send failed:', err.message); }
  );
});
```

### Pattern 5: Environment Variable Gating
**What:** The runner requires COMMS_PSK for WebSocket but degrades gracefully if not set. The watchdog still restarts Claude Code even without network connectivity.
**When to use:** When the watchdog must work independently of the comms layer.
**Important:** Unlike index.js which exits on missing PSK, the runner should log a warning and continue -- the watchdog function is more critical than the notification function.

### Anti-Patterns to Avoid
- **Timer-based cooldown:** Do NOT use setTimeout to schedule restart attempts. The poll loop already runs on a timer -- just skip cycles where `cooldown.ready()` returns false. This avoids timer stacking and is trivially testable.
- **Blocking on email send:** Never `await` the email execFile in the restart path. Fire-and-forget with error logging.
- **Modifying index.js:** index.js is the main comms client entry point. watchdog-runner.js is a separate process. Do NOT merge them.
- **Re-implementing auto-reconnect:** CommsClient already has exponential backoff reconnection. The runner just needs to call `connect()` once after restart -- CommsClient handles the rest.
- **Resetting cooldown on restart_success:** Only reset on self-test PASS (verified === true). The restart_success event fires before verification is complete.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Email sending | Custom SMTP/Gmail client | `execFile('node', [send_email.js, ...])` | send_email.js already handles OAuth, token refresh, Gmail API. One-liner invocation. |
| WebSocket reconnection | Custom reconnect logic in runner | `CommsClient.connect()` | CommsClient already has full exponential backoff auto-reconnect. Just call connect(). |
| Heartbeat lifecycle | Manual timer management | `HeartbeatSender` | Already wires start/stop to CommsClient open/close events. Existing pattern from index.js. |
| Process detection | Custom WMI/PowerShell detection | `defaultDetect()` in watchdog.js | Already implemented with tasklist, 5s timeout, null-on-error graceful degradation. |

**Key insight:** All supporting infrastructure already exists in the codebase. Phase 5 is about wiring, not building. The only new code is EscalatingCooldown (a ~30-line class) and the integration glue in watchdog-runner.js.

## Common Pitfalls

### Pitfall 1: Cooldown Reset on restart_success Instead of Self-Test Pass
**What goes wrong:** If you reset cooldown when `restart_success` fires, but the process dies 1 second later (detected at the 3s verify), you've already reset the cooldown. The next crash will start at step 0 instead of escalating.
**Why it happens:** `restart_success` fires at PID creation, before the 3s verify.
**How to avoid:** Only reset cooldown in the `self_test_passed` handler (after verify confirms process is alive). If the 3s verify fails, cooldown stays escalated.
**Warning signs:** Cooldown resets to 0 but process keeps dying -- rapid restart thrashing despite cooldown being "implemented."

### Pitfall 2: Timer Mock Interference with Poll + Cooldown
**What goes wrong:** Tests that mock setInterval (for the poll loop) and also need to test cooldown timing can have confusing interactions. Cooldown uses `Date.now()`, not setTimeout.
**Why it happens:** `t.mock.timers.tick()` advances setInterval but does NOT advance `Date.now()` unless you explicitly enable Date mocking.
**How to avoid:** EscalatingCooldown accepts `nowFn` for DI. Tests inject a fake clock that they control directly, independent of timer mocks. Alternatively, enable Date in mock.timers: `t.mock.timers.enable({ apis: ['setInterval', 'setTimeout', 'Date'] })`.
**Warning signs:** Cooldown always returns `ready() === false` in tests because `Date.now()` never advances.

### Pitfall 3: Email execFile Path on Windows
**What goes wrong:** send_email.js is at `C:/Users/bono/racingpoint/racecontrol/send_email.js` (different repo). If the path is hardcoded and the file moves, emails silently fail.
**Why it happens:** The send_email.js lives in the racecontrol repo, not comms-link.
**How to avoid:** Use an absolute path constant at the top of watchdog-runner.js. Log the error if execFile fails. Consider making the path configurable via environment variable (SEND_EMAIL_PATH) with a sensible default.
**Warning signs:** `[WATCHDOG] Email send failed: ENOENT` in logs.

### Pitfall 4: CommsClient connect() Before PSK Available
**What goes wrong:** If COMMS_PSK is not set in the Task Scheduler environment, CommsClient construction fails or connects without auth.
**Why it happens:** Environment variables set in the user shell may not be available in Task Scheduler tasks.
**How to avoid:** Check for PSK at startup. If missing, log a warning and skip WebSocket setup (watchdog still functions). The Task Scheduler registration script should ensure environment variables are passed.
**Warning signs:** Watchdog starts but never connects to Bono, no heartbeats, no email on restart.

### Pitfall 5: send_email.js Uses require() (CJS) Not import (ESM)
**What goes wrong:** send_email.js uses `require()` (CommonJS), while comms-link is ESM. You cannot `import` it.
**Why it happens:** send_email.js was written for the racecontrol repo which is CJS.
**How to avoid:** Invoke it as a separate process via `execFile('node', [path, args...])` -- not `import`. This is already the decided approach, but worth flagging: never try to import it directly.
**Warning signs:** `ERR_REQUIRE_ESM` or `SyntaxError: Cannot use import statement` errors.

### Pitfall 6: Cooldown Not Gating First Attempt
**What goes wrong:** If the cooldown gates the first-ever crash detection, the watchdog never restarts Claude Code on the very first crash.
**Why it happens:** Incorrectly initializing attemptCount to 1 or forgetting that ready() should return true when attemptCount is 0.
**How to avoid:** `ready()` returns true when attemptCount is 0 (no previous attempts). `recordAttempt()` is called just before restart. The delay only applies to subsequent attempts.
**Warning signs:** Claude crashes but watchdog sits idle for 5 seconds before first restart.

## Code Examples

Verified patterns from the existing codebase:

### Existing DI Pattern (from watchdog.js)
```javascript
// Constructor accepts injectable functions
constructor(options = {}) {
  super();
  this.#pollMs = options.pollMs ?? 3000;
  this.#detectFn = options.detectFn ?? defaultDetect;
  this.#killFn = options.killFn ?? defaultKill;
  this.#spawnFn = options.spawnFn ?? defaultSpawn;
  this.#findExeFn = options.findExeFn ?? findClaudeExe;
}
```

### Existing Timer Mock Pattern (from watchdog.test.js)
```javascript
it('emits crash_detected when detectFn returns false', async (t) => {
  t.mock.timers.enable({ apis: ['setInterval', 'setTimeout'] });

  const wd = new ClaudeWatchdog({
    pollMs: 3000,
    detectFn: async () => false,
    killFn: async () => {},
    spawnFn: async () => 999,
    findExeFn: () => '/fake/claude.exe',
  });

  wd.on('crash_detected', (evt) => events.push(evt));
  wd.start();

  // Microtask settling
  await Promise.resolve();
  await Promise.resolve();

  // Advance timers
  t.mock.timers.tick(2000);
  await Promise.resolve();
  await Promise.resolve();
});
```

### CommsClient Wiring Pattern (from index.js)
```javascript
const client = new CommsClient({ url, psk });
const heartbeat = new HeartbeatSender(client);

client.on('open', () => {
  heartbeat.start();
});

client.on('close', () => {
  heartbeat.stop();
});

client.connect();
```

### execFile Pattern for Email (adapted from existing patterns)
```javascript
import { execFile } from 'node:child_process';

// Fire-and-forget email
function sendRestartEmail(to, subject, body) {
  execFile('node', [SEND_EMAIL_PATH, to, subject, body],
    { timeout: 30000 },
    (err) => {
      if (err) console.error(`[WATCHDOG] Email send failed: ${err.message}`);
      else console.log('[WATCHDOG] Restart notification email sent');
    },
  );
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed retry delay | Escalating cooldown with steps array | Common pattern | Prevents restart thrashing on persistent failures |
| PID-only liveness check | PID + post-spawn verify (3s detection) | Phase 4 | Already implemented -- self-test enhances this with event emission |
| Separate watchdog and comms processes | Single runner owns both | Phase 5 | Runner becomes integration hub -- one Task Scheduler task handles everything |
| PowerShell watchdog (old) | Node.js ClaudeWatchdog + Task Scheduler | Phase 4 | Old PS watchdog preserved until Phase 5 confirms stability (per STATE.md decision) |

**Deprecated/outdated:**
- Old PowerShell watchdog: Preserved until Phase 5 confirms new watchdog stability. Can be retired after Phase 5 verification.

## Open Questions

1. **Environment variables in Task Scheduler**
   - What we know: COMMS_PSK and COMMS_URL are needed for WebSocket. The Task Scheduler task was registered in Phase 4 via register-watchdog.js.
   - What's unclear: Whether environment variables from the user profile are available in the scheduled task context. schtasks /create does not explicitly pass environment variables.
   - Recommendation: Test on the actual machine. If env vars are not available, the runner should read from a config file or the scheduled task command should set them inline. For now, gracefully degrade: if PSK is missing, skip WebSocket setup and log a warning. This can be fixed in verification without blocking implementation.

2. **send_email.js location stability**
   - What we know: Located at `C:/Users/bono/racingpoint/racecontrol/send_email.js`. Uses googleapis from the racingpoint-google package.
   - What's unclear: Whether this path will change if racecontrol is reorganized.
   - Recommendation: Use a constant at the top of watchdog-runner.js. Optionally support a SEND_EMAIL_PATH env var override. Log clearly if the file is not found.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node.js v22.14.0) |
| Config file | None -- uses package.json `test` script |
| Quick run command | `node --test test/watchdog.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WD-04 | Escalating cooldown steps (5s, 15s, 30s, 60s, 5min), clamping, ready(), reset() | unit | `node --test test/watchdog.test.js` | Extend existing |
| WD-04 | Poll loop skips restart when cooldown not ready | unit | `node --test test/watchdog.test.js` | Extend existing |
| WD-05 | Self-test emits self_test_passed when verify succeeds after restart | unit | `node --test test/watchdog.test.js` | Extend existing |
| WD-05 | Self-test failure does NOT reset cooldown | unit | `node --test test/watchdog.test.js` | Extend existing |
| WD-06 | CommsClient.connect() called after self-test pass (integration) | unit | `node --test test/watchdog.test.js` | New test |
| WD-07 | execFile called with correct args after self-test pass | unit | `node --test test/watchdog.test.js` | New test |

### Sampling Rate
- **Per task commit:** `node --test test/watchdog.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] EscalatingCooldown unit tests (new `describe` block in watchdog.test.js)
- [ ] WD-04 integration tests (cooldown + poll loop interaction)
- [ ] WD-05 self-test event tests (self_test_passed / self_test_failed)
- [ ] WD-06 CommsClient wiring tests (mock CommsClient, verify connect() called)
- [ ] WD-07 email notification tests (mock execFile, verify args)

Note: All new tests go in the existing `test/watchdog.test.js` file, following the pattern of adding new `describe` blocks per requirement. No new test files needed.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `james/watchdog.js` (229 lines), `james/watchdog-runner.js` (56 lines), `james/comms-client.js` (181 lines), `james/heartbeat-sender.js` (52 lines), `james/index.js` (49 lines)
- Existing tests: `test/watchdog.test.js` (555 lines, 15 test suites, 75 total tests passing)
- `racecontrol/send_email.js` (19 lines) -- CommonJS, takes 3 CLI args: to, subject, body
- CONTEXT.md -- All decisions locked, pattern adapted from racecontrol Phase 5

### Secondary (MEDIUM confidence)
- Node.js v22.14.0 docs: `node:test` mock.timers API, `node:child_process` execFile API
- racecontrol Phase 5 EscalatingBackoff pattern (referenced in CONTEXT.md, adapted for faster steps)

### Tertiary (LOW confidence)
- None -- all research verified against existing codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - zero new dependencies, all patterns already in codebase
- Architecture: HIGH - all integration points clearly defined by CONTEXT.md decisions, verified against existing code
- Pitfalls: HIGH - all pitfalls identified from direct codebase analysis (CJS/ESM mismatch, timer mock behavior, cooldown reset timing)

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable -- no fast-moving dependencies)
